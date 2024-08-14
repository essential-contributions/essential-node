use essential_types::Block;
use essential_types::{contract::Contract, ContentAddress};
use futures::stream::TryStreamExt;
use futures::{Stream, TryFutureExt};
use rusqlite_pool::tokio::AsyncConnectionPool;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::task::spawn_blocking;

pub(crate) use streams::stream_blocks;
pub(crate) use streams::stream_contracts;

use crate::error::{CriticalError, InternalResult, RecoverableError};
use crate::DataSyncError;

mod streams;
#[cfg(test)]
mod tests;

/// The progress of the contract sync.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContractProgress {
    /// The last L2 block number that was synced
    /// that contained any contract deployments.
    pub l2_block_number: u64,
    /// The address of the last contract that was synced
    /// from this block.
    pub last_contract: ContentAddress,
}

/// The progress of the block sync.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockProgress {
    /// The last l2 block number that was synced.
    pub last_block_number: u64,
    /// The address of the last block that was synced.
    /// Used to check for forks.
    pub last_block_hash: ContentAddress,
}

/// Get the last contract progress from the database.
pub async fn get_contract_progress(
    conn: &AsyncConnectionPool,
) -> crate::Result<Option<ContractProgress>> {
    let conn = conn.acquire().await?;
    tokio::task::spawn_blocking(move || {
        let progress = essential_node_db::get_contract_progress(&conn)?;
        Ok(progress.map(Into::into))
    })
    .await?
}

/// Get the last block progress from the database.
pub async fn get_block_progress(
    conn: &AsyncConnectionPool,
) -> crate::Result<Option<BlockProgress>> {
    let mut conn = conn.acquire().await?;
    tokio::task::spawn_blocking(move || {
        let tx = conn.transaction()?;
        let block = essential_node_db::get_latest_block(&tx)?;
        tx.finish()?;
        let progress = block.map(|block| BlockProgress {
            last_block_number: block.number,
            last_block_hash: essential_hash::content_addr(&block),
        });
        Ok(progress)
    })
    .await?
}

/// Sync contracts from the provided stream.
///
/// The first contract in the stream must be the last
/// contract that was synced unless progress is None.
pub async fn sync_contracts<S>(
    conn: Arc<AsyncConnectionPool>,
    progress: &Option<ContractProgress>,
    notify: watch::Sender<()>,
    stream: S,
) -> InternalResult<()>
where
    S: Stream<Item = InternalResult<Contract>>,
{
    tokio::pin!(stream);

    // The first contract in the stream must be the last
    // synced contract.
    //
    // If there is progress, check that the last contract
    // matches or return an error.
    //
    // This contract is skipped as it is already in the database.
    if let Some(progress) = progress {
        // Wait to get the first contract from the stream.
        let last = stream.try_next().await?;

        // Check that the contract matches the progress.
        check_contract_fork(&last, progress)?;
    }

    // Get the `l2_block_number` for the next contract.
    //
    // Note this is specific to the server implementation.
    // In the future this will be changed.
    let mut l2_block_number = match progress {
        Some(ContractProgress {
            l2_block_number, ..
        }) => l2_block_number.saturating_add(1),
        None => 0,
    };

    stream
        .try_for_each(move |contract| {
            // The `l2_block_number` for this contract.
            let this_l2_block_number = l2_block_number;

            // Increment the `l2_block_number` for the next contract.
            //
            // Note this is specific to the server implementation.
            // In the future this will be changed.
            l2_block_number += 1;

            // Write the contract to the database.
            let conn = conn.clone();
            let notify = notify.clone();
            async move {
                write_contract(&conn, contract, this_l2_block_number, notify.clone())
                    .map_err(Into::into)
                    .await
            }
        })
        .await?;
    Ok(())
}

/// Sync blocks from the provided stream.
///
/// The first block in the stream must be the last
/// block that was synced unless progress is None.
pub async fn sync_blocks<S>(
    conn: Arc<AsyncConnectionPool>,
    progress: &Option<BlockProgress>,
    notify: watch::Sender<()>,
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
            let conn = conn.clone();
            async move {
                // If the block is not sequential, return an error.
                if !sequential_block {
                    return Err(
                        RecoverableError::NonSequentialBlock(block_number, block.number).into(),
                    );
                }

                // Write the block to the database.
                write_block(&conn, block).await?;

                // Best effort to notify of new block
                let _ = notify.send(());
                Ok(())
            }
        })
        .await?;
    Ok(())
}

/// Write a contract and progress atomically to the database.
async fn write_contract(
    conn: &AsyncConnectionPool,
    contract: Contract,
    l2_block_number: u64,
    notify: watch::Sender<()>,
) -> crate::Result<()> {
    let mut conn = conn.acquire().await?;
    spawn_blocking::<_, rusqlite::Result<_>>(move || {
        // Calculate the contract hash for the progress.
        let contract_hash = essential_hash::contract_addr::from_contract(&contract);

        let tx = conn.transaction()?;
        essential_node_db::insert_contract(&tx, &contract, l2_block_number)?;
        // Technically we don't currently need to store the progress.
        // We could instead do what we do with blocks and just take the max.
        // However, in the future it will be possible to have multiple contracts
        // at the same `l2_block_number` and we will need this to know where we were up to.
        essential_node_db::insert_contract_progress(&tx, l2_block_number, &contract_hash)?;
        tx.commit()?;
        Ok(conn)
    })
    .await??;
    // Best effort to notify of new contract
    let _ = notify.send(());
    Ok(())
}

/// Write a block to the database.
async fn write_block(conn: &AsyncConnectionPool, block: Block) -> crate::Result<()> {
    let mut conn = conn.acquire().await?;
    spawn_blocking(move || {
        let tx = conn.transaction()?;
        essential_node_db::insert_block(&tx, &block)?;
        tx.commit()?;
        Ok(())
    })
    .await?
}

/// Check that the contract matches the last progress.
fn check_contract_fork(
    contract: &Option<Contract>,
    progress: &ContractProgress,
) -> crate::Result<()> {
    match contract {
        Some(contract) => {
            let contract_hash = essential_hash::contract_addr::from_contract(contract);
            if contract_hash != progress.last_contract {
                // There was a contract but it didn't match the expected contract.
                return Err(CriticalError::DataSyncFailed(
                    DataSyncError::ContractMismatch(
                        progress.l2_block_number,
                        progress.last_contract.clone(),
                        Some(contract_hash),
                    ),
                ));
            }
        }
        None => {
            // There was expected to be a contract but there was none.
            return Err(CriticalError::DataSyncFailed(
                DataSyncError::ContractMismatch(
                    progress.l2_block_number,
                    progress.last_contract.clone(),
                    None,
                ),
            ));
        }
    }

    Ok(())
}

/// Check that the block matches the last progress.
fn check_block_fork(block: &Option<Block>, progress: &BlockProgress) -> crate::Result<()> {
    match block {
        Some(block) => {
            let block_hash = essential_hash::content_addr(block);
            if block_hash != progress.last_block_hash {
                // There was a block but it didn't match the expected block.
                return Err(CriticalError::DataSyncFailed(DataSyncError::Fork(
                    progress.last_block_number,
                    progress.last_block_hash.clone(),
                    Some(block_hash),
                )));
            }
        }
        None => {
            // There was expected to be a block but there was none.
            return Err(CriticalError::DataSyncFailed(DataSyncError::Fork(
                progress.last_block_number,
                progress.last_block_hash.clone(),
                None,
            )));
        }
    }

    Ok(())
}

impl From<(u64, ContentAddress)> for ContractProgress {
    fn from((l2_block_number, last_contract): (u64, ContentAddress)) -> Self {
        Self {
            l2_block_number,
            last_contract,
        }
    }
}
