use crate::{
    db::ConnectionPool,
    error::{CriticalError, InternalError, RecoverableError, ValidationError},
    handles::validation::Handle,
    validate::{self, InvalidOutcome, ValidOutcome, ValidateOutcome},
};
use essential_hash::content_addr;
use essential_node_db::{
    get_block_number, get_latest_finalized_block_address, update_validation_progress,
};
use essential_types::{Block, ContentAddress};
use futures::stream::{StreamExt, TryStreamExt};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

#[cfg(test)]
mod tests;

/// Run the stream that validates blocks.
///
/// The stream is spawned and run in the background.
/// The watch channel listens to notifications when a new block is added to the database.
///
/// Returns a handle that can be used to clone or join the stream.
///
/// Recoverable errors will be logged and the stream will be restarted.
/// Critical errors will cause the stream to end.
pub fn validation_stream(
    conn_pool: ConnectionPool,
    contract_registry: ContentAddress,
    block_rx: watch::Receiver<()>,
) -> Result<Handle<CriticalError>, CriticalError> {
    let (shutdown, stream_close) = watch::channel(());

    let jh = tokio::spawn(async move {
        let mut missing_block = false;
        loop {
            let (self_notify, self_notify_rx) = watch::channel(());
            let rx = if missing_block {
                WatchStream::from_changes(block_rx.clone())
            } else {
                WatchStream::new(block_rx.clone())
            };
            let rx = futures::stream::select(rx, WatchStream::from_changes(self_notify_rx));
            let mut stream_close = stream_close.clone();
            let close = async move {
                let _ = stream_close.changed().await;
            };

            let r = rx
                .take_until(close)
                .map(Ok)
                .try_for_each(|_| {
                    validate_next_block(conn_pool.clone(), &contract_registry, self_notify.clone())
                })
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

#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
async fn validate_next_block(
    conn_pool: ConnectionPool,
    contract_registry: &ContentAddress,
    self_notify: watch::Sender<()>,
) -> Result<(), InternalError> {
    let progress = get_last_progress(&conn_pool).await?;

    let block = get_next_block(&conn_pool, progress).await?;
    let block_address = content_addr(&block);

    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Validating block {} with number {}",
        block_address,
        block.number
    );

    let res = validate::validate(&conn_pool, contract_registry, &block).await?;

    match res {
        // Validation was successful.
        ValidateOutcome::Valid(ValidOutcome {
            total_gas: _total_gas,
        }) => {
            let mut conn = conn_pool.acquire().await.map_err(CriticalError::from)?;
            let r: Result<(), InternalError> = tokio::task::spawn_blocking(move || {
                // Update validation progress.
                update_validation_progress(&conn, &block_address).map_err(ValidationError::from)?;
                let tx = conn.transaction();
                // Keep validating if there are more blocks awaiting.
                let latest_finalized_block_number = tx.and_then(|tx| {
                    get_latest_finalized_block_address(&tx).and_then(|hash| {
                        hash.and_then(|hash| get_block_number(&tx, &hash).transpose())
                            .transpose()
                    })
                });
                if let Ok(Some(latest_block_number)) = latest_finalized_block_number {
                    if latest_block_number > block.number {
                        let _ = self_notify.send(());
                    }
                }
                Ok(())
            })
            .await
            .map_err(RecoverableError::Join)?;
            r
        }
        // Validation failed.
        ValidateOutcome::Invalid(InvalidOutcome {
            failure: _failure,
            solution_index,
        }) => {
            // Insert the failed solution into the database.
            let failed_solution = content_addr(
                block
                    .solutions
                    .get(solution_index)
                    .expect("Failed solution must exist."),
            );
            let conn = conn_pool.acquire().await.map_err(CriticalError::from)?;
            tokio::task::spawn_blocking(move || {
                essential_node_db::insert_failed_block(&conn, &block_address, &failed_solution)
                    .map_err(ValidationError::from)?;

                Ok(())
            })
            .await
            .map_err(RecoverableError::Join)?
        }
    }?;

    Ok(())
}

async fn get_last_progress(
    conn: &ConnectionPool,
) -> Result<Option<ContentAddress>, RecoverableError> {
    conn.get_validation_progress()
        .await
        .map_err(|_err| RecoverableError::LastProgress)
}

async fn get_next_block(
    conn: &ConnectionPool,
    progress: Option<ContentAddress>,
) -> Result<Block, InternalError> {
    let mut conn = conn.acquire().await.map_err(CriticalError::from)?;
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

            let blocks = essential_node_db::list_unchecked_blocks(&tx, range)
                .map_err(RecoverableError::from)?;

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
            // Make sure the block is inserted into the database before validating
            let current_block = iter
                .next()
                .ok_or(RecoverableError::BlockNotFound(hash.clone()))?;
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

fn check_missing_block(e: &InternalError) -> bool {
    matches!(
        e,
        InternalError::Recoverable(RecoverableError::FirstBlockNotFound)
            | InternalError::Recoverable(RecoverableError::BlockNotFound(_))
    )
}

/// Exit on critical errors, log recoverable errors.
fn handle_error(e: InternalError) -> Result<(), CriticalError> {
    let e = map_recoverable_errors(e);
    match e {
        InternalError::Critical(e) => {
            #[cfg(feature = "tracing")]
            tracing::error!(
                "The validation stream has encountered a critical error: {} and cannot recover.",
                e
            );
            Err(e)
        }
        #[cfg(feature = "tracing")]
        InternalError::Recoverable(e) => {
            tracing::error!("The validation stream has encountered a recoverable error: {} and will now restart the stream.", e);

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
