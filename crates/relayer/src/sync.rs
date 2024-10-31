use essential_node_db::{self as db, with_tx, ConnectionPool};
use essential_node_types::BlockTx;
use essential_types::Block;
use essential_types::ContentAddress;
use essential_types::Word;
use futures::stream::TryStreamExt;
use futures::Stream;

pub(crate) use streams::stream_blocks;

use crate::error::{CriticalError, InternalResult, RecoverableError};
use crate::DataSyncError;

mod streams;

/// The progress of the block sync.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockProgress {
    /// The last l2 block number that was synced.
    pub last_block_number: Word,
    /// The address of the last block that was synced.
    /// Used to check for forks.
    pub last_block_address: ContentAddress,
}

/// Get the last block progress from the database.
pub async fn get_block_progress(
    pool: &ConnectionPool,
) -> Result<Option<BlockProgress>, db::pool::AcquireThenRusqliteError> {
    pool.acquire_then(|conn| {
        with_tx(conn, |tx| {
            let Some(block_address) = db::get_latest_finalized_block_address(tx)? else {
                return Ok(None);
            };
            let Some((block_number, _ts)) = db::get_block_header(tx, &block_address)? else {
                return Ok(None);
            };
            Ok(Some(BlockProgress {
                last_block_number: block_number,
                last_block_address: block_address,
            }))
        })
    })
    .await
}

#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
/// Sync blocks from the provided stream.
///
/// The first block in the stream must be the last
/// block that was synced unless progress is None.
pub async fn sync_blocks<S>(
    pool: ConnectionPool,
    progress: &Option<BlockProgress>,
    notify: BlockTx,
    stream: S,
) -> InternalResult<()>
where
    S: Stream<Item = InternalResult<Block>>,
{
    tokio::pin!(stream);

    // The first block in the stream must be the last
    // synced block.
    //
    // If there is progress, check that the last block
    // matches or return an error.
    //
    // This block is skipped as it is already in the database.
    if let Some(progress) = progress {
        // Wait to get the first block from the stream.
        let last = stream.try_next().await?;

        // Check that the block matches the progress.
        check_block_fork(&last, progress)?;
    }

    // Increment the block number to get the next block's number.
    let mut block_number = match progress {
        Some(BlockProgress {
            last_block_number, ..
        }) => last_block_number.saturating_add(1),
        None => 0,
    };

    stream
        .try_for_each(move |block| {
            // Check this block is the expect `N + 1`.
            let sequential_block = block.number == block_number;

            // Increment the block number for the next block.
            block_number = block.number.saturating_add(1);

            let notify = notify.clone();
            let pool = pool.clone();
            async move {
                // If the block is not sequential, return an error.
                if !sequential_block {
                    return Err(
                        RecoverableError::NonSequentialBlock(block_number, block.number).into(),
                    );
                }

                #[cfg(feature = "tracing")]
                tracing::debug!("Writing block number {} to database", block.number);

                // Write the block to the database.
                write_block(&pool, block)
                    .await
                    .map_err(CriticalError::from)?;

                // Best effort to notify of new block
                notify.notify();
                Ok(())
            }
        })
        .await?;
    Ok(())
}

/// Write a block to the database.
async fn write_block(
    pool: &ConnectionPool,
    block: Block,
) -> Result<(), db::pool::AcquireThenRusqliteError> {
    pool.acquire_then(move |conn| {
        with_tx(conn, |tx| {
            let block_address = essential_hash::content_addr(&block);
            essential_node_db::insert_block(tx, &block)?;

            // We are currently finalizing the block immediately.
            // This will be changed in the when we have a time period
            // before finalization can occur.
            essential_node_db::finalize_block(tx, &block_address)?;
            Ok(())
        })
    })
    .await
}

/// Check that the block matches the last progress.
fn check_block_fork(block: &Option<Block>, progress: &BlockProgress) -> crate::Result<()> {
    match block {
        Some(block) => {
            let block_address = essential_hash::content_addr(block);
            if block_address != progress.last_block_address {
                // There was a block but it didn't match the expected block.
                return Err(CriticalError::DataSyncFailed(DataSyncError::Fork(
                    progress.last_block_number,
                    progress.last_block_address.clone(),
                    Some(block_address),
                )));
            }
        }
        None => {
            // There was expected to be a block but there was none.
            return Err(CriticalError::DataSyncFailed(DataSyncError::Fork(
                progress.last_block_number,
                progress.last_block_address.clone(),
                None,
            )));
        }
    }

    Ok(())
}
