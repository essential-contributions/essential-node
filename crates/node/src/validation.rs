use crate::{
    db::{
        self, get_block_header, get_latest_finalized_block_address, update_validation_progress, ConnectionPool,
    },
    error::{CriticalError, InternalError, RecoverableError, ValidationError},
    handles::validation::Handle,
    validate::{self, InvalidOutcome, ValidOutcome, ValidateOutcome},
};
use essential_hash::content_addr;
use essential_node_db::{QueryError};
use essential_node_types::block_notify::BlockRx;
use essential_types::{Block, ContentAddress};
use tokio::sync::watch;

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
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn validation_stream(
    conn_pool: ConnectionPool,
    contract_registry: ContentAddress,
    mut block_rx: BlockRx,
) -> Result<Handle<CriticalError>, CriticalError> {
    let (shutdown, stream_close) = watch::channel(());

    let jh = tokio::spawn(async move {
        let mut stream_close = stream_close.clone();
        loop {
            let err = 'wait_next_block: loop {
                loop {
                    match validate_next_block(conn_pool.clone(), &contract_registry).await {
                        Err(err) => break 'wait_next_block err,
                        Ok(more_blocks_left) => {
                            if more_blocks_left {
                                continue;
                            } else {
                                #[cfg(feature = "tracing")]
                                tracing::debug!(
                                    "Reached head of chain: Awaiting new block notifications"
                                );
                                break;
                            }
                        }
                    }
                }

                // Await a block notification or stream close.
                tokio::select! {
                    _ = stream_close.changed() => return Ok(()),
                    res = block_rx.changed() => if let Err(_err) = res {
                        #[cfg(feature = "tracing")]
                        tracing::debug!("Block notification channel dropped: closing stream");
                        return Ok(());
                    },
                }
            };

            // Return error if it's critical or
            // continue if it's recoverable
            handle_error(err)?;
        }
    });

    Ok(Handle::new(jh, shutdown))
}

/// Validate the next block and return whether or not there are more blocks available in the DB to
/// validate.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
async fn validate_next_block(
    conn_pool: ConnectionPool,
    contract_registry: &ContentAddress,
) -> Result<bool, InternalError> {
    let progress = get_last_progress(&conn_pool)
        .await?
        .ok_or(CriticalError::LastProgressNone)?;

    // Only validate if there's a block to validate.
    let Some(block) = get_next_block(&conn_pool, progress).await? else {
        return Ok(false);
    };
    let block_address = content_addr(&block);

    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Validating block {} with number {}",
        block_address,
        block.number
    );

    let res = validate::validate(&conn_pool, contract_registry, &block).await?;

    let more_blocks_available = match res {
        // Validation was successful.
        ValidateOutcome::Valid(ValidOutcome {
            total_gas: _total_gas,
        }) => {
            let mut conn = conn_pool.acquire().await.map_err(CriticalError::from)?;
            let r: Result<bool, InternalError> = tokio::task::spawn_blocking(move || {
                // Update validation progress.
                update_validation_progress(&conn, &block_address).map_err(ValidationError::from)?;
                let tx = conn.transaction();
                // Keep validating if there are more blocks awaiting.
                let latest_finalized_block_number = tx.and_then(|tx| {
                    get_latest_finalized_block_address(&tx).and_then(|hash| {
                        hash.and_then(|hash| {
                            get_block_header(&tx, &hash)
                                .map(|opt| opt.map(|(number, _ts)| number))
                                .transpose()
                        })
                        .transpose()
                    })
                });
                if let Ok(Some(latest_block_number)) = latest_finalized_block_number {
                    if latest_block_number > block.number {
                        return Ok(true);
                    }
                }
                Ok(false)
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
            let block_address = block_address.clone();
            let r: Result<bool, InternalError> = conn_pool
                .acquire_then(move |conn| {
                    db::insert_failed_block(conn, &block_address, &failed_solution)
                        .map_err(ValidationError::from)
                        .map(|_| Ok(false))
                })
                .await
                .map_err(InternalError::from)?;
            r
        }
    }?;

    Ok(more_blocks_available)
}

async fn get_last_progress(
    conn: &ConnectionPool,
) -> Result<Option<ContentAddress>, RecoverableError> {
    conn.get_validation_progress()
        .await
        .map_err(RecoverableError::from)
}

/// Get the next block, or return `None` if we've reached the head of the chain.
async fn get_next_block(
    conn: &ConnectionPool,
    progress: ContentAddress,
) -> Result<Option<Block>, InternalError> {
    conn.acquire_then(move |conn| {
        let blocks = essential_node_db::get_next_block_addresses(conn, &progress)?;
        if blocks.is_empty() {
            Ok::<_, QueryError>(Ok(None))
        } else if blocks.len() > 1 {
            Ok(Err(CriticalError::Fork.into()))
        } else {
            let block_address = blocks.into_iter().next().expect("blocks is not empty");
            let tx = conn.transaction()?;
            let block = essential_node_db::get_block(&tx, &block_address)?;
            Ok(Ok(block))
        }
    })
    .await
    .map_err(CriticalError::GetNextBlock)?
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
            db::pool::AcquireThenError::Acquire(e) => CriticalError::DbPoolClosed(e).into(),
            e @ db::pool::AcquireThenError::Join(_) => RecoverableError::ReadState(e).into(),
            db::pool::AcquireThenError::Inner(db::QueryError::Rusqlite(rus)) => {
                if is_recoverable_db_err(&rus) {
                    RecoverableError::Rusqlite(rus).into()
                } else {
                    CriticalError::DatabaseFailed(rus).into()
                }
            }
            db::pool::AcquireThenError::Inner(essential_node_db::QueryError::UnsupportedRange) => {
                RecoverableError::Query(essential_node_db::QueryError::UnsupportedRange).into()
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
