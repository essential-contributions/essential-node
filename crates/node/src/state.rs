use crate::{
    db::{ConnectionHandle, ConnectionPool},
    error::{CriticalError, InternalError, RecoverableError},
    state_handle::Handle,
};
use essential_node_db::{
    get_block_number, get_latest_finalized_block_hash, hash_block_and_solutions, update_state,
    update_state_progress, BlockHash,
};
use essential_types::{solution::Mutation, Block, ContentAddress};
use futures::stream::{StreamExt, TryStreamExt};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

#[cfg(test)]
mod tests;

/// Mutations per contract address to be applied to the state.
pub struct Mutations<I>
where
    I: IntoIterator<Item = Mutation>,
{
    pub contract_address: ContentAddress,
    pub mutations: I,
}

/// Run the stream that derives state from blocks.
///
/// The stream is spawned and run in the background.
/// The watch channel listens to notifications when a new block is added to the database.
///
/// Returns a handle that can be used to clone or join the stream.
///
/// Recoverable errors will be logged and the stream will be restarted.
/// Critical errors will cause the stream to end.
pub fn derive_state_stream(
    conn: ConnectionPool,
    block_rx: watch::Receiver<()>,
    self_notify: watch::Sender<()>,
) -> Result<Handle<CriticalError>, CriticalError> {
    let (shutdown, stream_close) = watch::channel(());

    let jh = tokio::spawn(async move {
        let mut missing_block = false;
        loop {
            let rx = if missing_block {
                WatchStream::from_changes(block_rx.clone())
            } else {
                WatchStream::new(block_rx.clone())
            };
            let mut stream_close = stream_close.clone();
            let close = async move {
                let _ = stream_close.changed().await;
            };

            let r = rx
                .take_until(close)
                .map(Ok)
                .try_for_each(|_| derive_next_block_state(conn.clone(), self_notify.clone()))
                .await;

            match r {
                // Stream has ended, return from the task
                Ok(_) => return Ok(()),
                Err(e) => {
                    missing_block = check_missing_block(&e);
                    // Return error if it's critical or
                    // continue if it's recoverable
                    handle_error(e)?;
                }
            }
        }
    });

    Ok(Handle::new(jh, shutdown))
}

fn check_missing_block(e: &InternalError) -> bool {
    matches!(
        e,
        InternalError::Recoverable(RecoverableError::BlockNotFound(_))
    )
}

#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
/// Apply state mutations to the next block in the database.
///
/// Read the last progress on state updates.
/// Get the next block to process and apply its state mutations.
async fn derive_next_block_state(
    conn: ConnectionPool,
    self_notify: watch::Sender<()>,
) -> Result<(), InternalError> {
    let progress = get_last_progress(&conn).await?;

    let block = get_next_block(&conn, progress).await?;

    #[cfg(feature = "tracing")]
    tracing::debug!("Deriving state for block number {}", block.number);

    let block_hash = hash_block_and_solutions(&block).0;

    let mutations = block.solutions.into_iter().flat_map(|solution| {
        solution.data.into_iter().map(|data| Mutations {
            contract_address: data.predicate_to_solve.contract,
            mutations: data.state_mutations,
        })
    });

    if update_state_in_db(conn, mutations, block.number, block_hash).await? {
        let _ = self_notify.send(());
    }
    Ok(())
}

/// Fetch the last processed block from the database in a blocking task.
async fn get_last_progress(
    conn: &ConnectionPool,
) -> Result<Option<(u64, BlockHash)>, RecoverableError> {
    conn.get_state_progress()
        .await
        .map_err(|_err| RecoverableError::LastProgress)
}

/// Fetch the next block to process from the database in a blocking task.
async fn get_next_block(
    conn: &ConnectionPool,
    progress: Option<(u64, BlockHash)>,
) -> Result<Block, InternalError> {
    let range = progress.as_ref().map_or(0..1, |(block_num, _)| {
        *block_num..block_num.saturating_add(2) // List previous and current block
    });

    let blocks = conn
        .list_blocks(range)
        .await
        .map_err(RecoverableError::ReadState)?;

    let block = match progress {
        // Get the next block
        Some((number, hash)) => {
            let mut iter = blocks.into_iter();
            let previous_block = iter.next().ok_or(CriticalError::Fork)?;
            // Make sure the block is inserted into the database before deriving state
            let current_block = iter.next().ok_or(RecoverableError::BlockNotFound(number))?;
            if hash_block_and_solutions(&previous_block).0 != hash {
                return Err(CriticalError::Fork.into());
            }
            current_block
        }
        // No progress, get the first block
        None => blocks
            .into_iter()
            .next()
            .ok_or(RecoverableError::BlockNotFound(0))?,
    };

    Ok(block)
}

/// Apply state mutations to the database in a blocking task.
#[cfg_attr(feature = "tracing", tracing::instrument("state_progress", skip_all))]
async fn update_state_in_db<S, I>(
    conn: ConnectionPool,
    mutations: I,
    block_number: u64,
    block_hash: BlockHash,
) -> Result<bool, InternalError>
where
    S: IntoIterator<Item = Mutation>,
    I: IntoIterator<Item = Mutations<S>> + Send + 'static,
{
    #[cfg(feature = "tracing")]
    let span = tracing::Span::current();

    let mut conn = conn.acquire().await.map_err(CriticalError::from)?;
    let r: Result<bool, InternalError> = tokio::task::spawn_blocking(move || {
        #[cfg(feature = "tracing")]
        let _guard = span.enter();

        update_state_in_db_inner(&mut conn, mutations, block_number, block_hash)?;
        let tx = conn.transaction();
        let latest_finalized_block_number = tx.and_then(|tx| {
            get_latest_finalized_block_hash(&tx).and_then(|hash| {
                hash.and_then(|hash| get_block_number(&tx, &hash).transpose())
                    .transpose()
            })
        });
        match latest_finalized_block_number {
            Ok(Some(latest_block_number)) => {
                if latest_block_number > block_number {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    })
    .await
    .map_err(RecoverableError::Join)?;
    r
}

fn update_state_in_db_inner<S, I>(
    conn: &mut ConnectionHandle,
    mutations: I,
    block_number: u64,
    block_hash: BlockHash,
) -> Result<(), CriticalError>
where
    S: IntoIterator<Item = Mutation>,
    I: IntoIterator<Item = Mutations<S>> + Send + 'static,
{
    let tx = conn.transaction()?;

    for mutation in mutations {
        for m in mutation.mutations {
            update_state(&tx, &mutation.contract_address, &m.key, &m.value)?;
        }
    }

    update_state_progress(&tx, block_number, &block_hash)?;

    tx.commit()?;

    #[cfg(feature = "tracing")]
    tracing::debug!(number = block_number, hash = %block_hash);

    Ok(())
}

/// Exit on critical errors, log recoverable errors.
fn handle_error(e: InternalError) -> Result<(), CriticalError> {
    let e = map_recoverable_errors(e);
    match e {
        InternalError::Critical(e) => {
            #[cfg(feature = "tracing")]
            tracing::error!(
                "The state derivation stream has encountered a critical error: {} and cannot recover.",
                e
            );
            Err(e)
        }
        #[cfg(feature = "tracing")]
        InternalError::Recoverable(e) => {
            tracing::error!("The state derivation stream has encountered a recoverable error: {} and will now restart the stream.", e);

            Ok(())
        }
        #[cfg(not(feature = "tracing"))]
        InternalError::Recoverable(_) => Ok(()),
    }
}

/// Some critical error types contain variants that we should handle as recoverable errors.
/// This function maps those errors to recoverable errors.
fn map_recoverable_errors(e: InternalError) -> InternalError {
    // Map recoverable rusqlite errors to recoverable errors
    match e {
        InternalError::Critical(CriticalError::DatabaseFailed(e)) => {
            if is_recoverable_db_err(&e) {
                RecoverableError::Rusqlite(e).into()
            } else {
                CriticalError::DatabaseFailed(e).into()
            }
        }
        InternalError::Critical(CriticalError::ReadState(e)) => match e {
            crate::db::AcquireThenError::Acquire(e) => CriticalError::DbPoolClosed(e).into(),
            e @ crate::db::AcquireThenError::Join(_) => RecoverableError::ReadState(e).into(),
            crate::db::AcquireThenError::Inner(essential_node_db::QueryError::Rusqlite(rus)) => {
                if is_recoverable_db_err(&rus) {
                    RecoverableError::Rusqlite(rus).into()
                } else {
                    CriticalError::DatabaseFailed(rus).into()
                }
            }
            e @ crate::db::AcquireThenError::Inner(essential_node_db::QueryError::Decode(_)) => {
                CriticalError::from(e).into()
            }
        },
        _ => e,
    }
}

fn is_recoverable_db_err(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ffi::ErrorCode::DatabaseLocked,
                ..
            },
            _
        ) | rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ffi::ErrorCode::DatabaseBusy,
                ..
            },
            _
        )
    )
}
