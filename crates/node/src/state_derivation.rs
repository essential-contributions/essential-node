use crate::{
    db::{ConnectionHandle, ConnectionPool},
    error::{CriticalError, InternalError, RecoverableError},
    handles::state::Handle,
};
use essential_hash::content_addr;
use essential_node_db::{
    get_block_number, get_latest_finalized_block_address, update_state, update_state_progress,
};
use essential_types::{solution::Mutation, Block, ContentAddress, Word};
use tokio::sync::watch;

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
pub fn state_derivation_stream(
    conn_pool: ConnectionPool,
    mut block_rx: watch::Receiver<()>,
) -> Result<Handle<CriticalError>, CriticalError> {
    let (shutdown, stream_close) = watch::channel(());

    let jh = tokio::spawn(async move {
        let mut stream_close = stream_close.clone();
        loop {
            let err = 'wait_next_block: loop {
                // Await a block notification or stream close.
                tokio::select! {
                    _ = stream_close.changed() => return Ok(()),
                    _ = block_rx.changed() => (),
                }

                loop {
                    match derive_next_block_state(conn_pool.clone()).await {
                        Err(err) => break 'wait_next_block err,
                        Ok(more_blocks_left) => {
                            if more_blocks_left {
                                continue;
                            } else {
                                break;
                            }
                        }
                    }
                }
            };

            // Return error if it's critical or
            // continue if it's recoverable
            handle_error(err)?;
        }
    });

    Ok(Handle::new(jh, shutdown))
}

#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
/// Apply state mutations to the next block in the database.
///
/// Read the last progress on state updates.
/// Get the next block to process and apply its state mutations.
///
/// Returns whether or not there are more blocks available in the DB to be derived.
async fn derive_next_block_state(conn_pool: ConnectionPool) -> Result<bool, InternalError> {
    let progress = get_last_progress(&conn_pool).await?;

    let block = get_next_block(&conn_pool, progress).await?;

    #[cfg(feature = "tracing")]
    tracing::debug!("Deriving state for block number {}", block.number);

    let block_address = content_addr(&block);

    let mutations = block.solutions.into_iter().flat_map(|solution| {
        solution.data.into_iter().map(|data| Mutations {
            contract_address: data.predicate_to_solve.contract,
            mutations: data.state_mutations,
        })
    });

    if update_state_in_db(conn_pool, mutations, block.number, block_address).await? {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Fetch the last processed block from the database in a blocking task.
async fn get_last_progress(
    conn_pool: &ConnectionPool,
) -> Result<Option<ContentAddress>, RecoverableError> {
    conn_pool
        .get_state_progress()
        .await
        .map_err(|_err| RecoverableError::LastProgress)
}

/// Fetch the next block to process from the database in a blocking task.
async fn get_next_block(
    conn_pool: &ConnectionPool,
    progress: Option<ContentAddress>,
) -> Result<Block, InternalError> {
    let mut conn = conn_pool.acquire().await.map_err(CriticalError::from)?;
    let blocks = tokio::task::spawn_blocking::<_, Result<_, InternalError>>({
        let progress = progress.clone();
        move || {
            let tx = conn.transaction().map_err(RecoverableError::Rusqlite)?;
            let range = match &progress {
                Some(block_address) => {
                    let block_number = essential_node_db::get_block_number(&tx, block_address)
                        .map_err(RecoverableError::Rusqlite)?
                        .ok_or(RecoverableError::BlockNotFound(block_address.clone()))?;
                    block_number..block_number.saturating_add(2)
                }
                None => 0..1,
            };

            let blocks =
                essential_node_db::list_blocks(&tx, range).map_err(RecoverableError::from)?;

            Ok(blocks)
        }
    })
    .await
    .map_err(RecoverableError::Join)??;

    let block = match progress {
        // Get the next block
        Some(hash) => {
            let mut iter = blocks.into_iter();
            let previous_block = iter.next().ok_or(CriticalError::Fork)?;
            // Make sure the block is inserted into the database before deriving state
            let current_block = iter
                .next()
                .ok_or(RecoverableError::NextBlockNotFound(hash.clone()))?;
            if content_addr(&previous_block) != hash {
                return Err(CriticalError::Fork.into());
            }
            current_block
        }
        // No progress, get the first block
        None => blocks
            .into_iter()
            .next()
            .ok_or(RecoverableError::FirstBlockNotFound)?,
    };

    Ok(block)
}

/// Apply state mutations to the database in a blocking task.
#[cfg_attr(feature = "tracing", tracing::instrument("state_progress", skip_all))]
async fn update_state_in_db<S, I>(
    conn_pool: ConnectionPool,
    mutations: I,
    block_number: Word,
    block_address: ContentAddress,
) -> Result<bool, InternalError>
where
    S: IntoIterator<Item = Mutation>,
    I: IntoIterator<Item = Mutations<S>> + Send + 'static,
{
    #[cfg(feature = "tracing")]
    let span = tracing::Span::current();

    let mut conn = conn_pool.acquire().await.map_err(CriticalError::from)?;
    let r: Result<bool, InternalError> = tokio::task::spawn_blocking(move || {
        #[cfg(feature = "tracing")]
        let _guard = span.enter();

        update_state_in_db_inner(&mut conn, mutations, block_number, block_address)?;
        let tx = conn.transaction();
        let latest_finalized_block_number = tx.and_then(|tx| {
            get_latest_finalized_block_address(&tx).and_then(|hash| {
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
    block_number: Word,
    block_address: ContentAddress,
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

    update_state_progress(&tx, &block_address)?;

    tx.commit()?;

    #[cfg(feature = "tracing")]
    tracing::debug!(number = block_number, hash = %block_address);

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
