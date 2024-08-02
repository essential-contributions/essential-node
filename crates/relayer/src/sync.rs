use std::borrow::{Borrow, BorrowMut};

use essential_types::Block;
use essential_types::{contract::Contract, ContentAddress};
use futures::stream::TryStreamExt;
use futures::{Stream, TryFutureExt};
use tokio::sync::watch;
use tokio::task::spawn_blocking;

pub(crate) use streams::check_for_block_fork;
pub(crate) use streams::check_for_contract_mismatch;
pub(crate) use streams::stream_blocks;
pub(crate) use streams::stream_contracts;

use crate::error::{InternalResult, RecoverableError};

mod streams;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContractProgress {
    pub l2_block_number: u64,
    pub last_contract: ContentAddress,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockProgress {
    pub last_block_number: u64,
    pub last_block_hash: ContentAddress,
}

pub struct WithConn<C, T> {
    pub conn: C,
    pub value: T,
}

pub async fn get_contract_progress<C>(
    conn: C,
) -> crate::Result<WithConn<C, Option<ContractProgress>>>
where
    C: Borrow<rusqlite::Connection> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let progress = essential_node_db::get_contract_progress(conn.borrow())?;
        Ok(WithConn {
            conn,
            value: progress.map(Into::into),
        })
    })
    .await?
}

pub async fn get_block_progress<C>(mut conn: C) -> crate::Result<WithConn<C, Option<BlockProgress>>>
where
    C: BorrowMut<rusqlite::Connection> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let tx = conn.borrow_mut().transaction()?;
        let block = essential_node_db::get_latest_block(&tx)?;
        tx.finish()?;
        let progress = block.map(|block| BlockProgress {
            last_block_number: block.number,
            last_block_hash: essential_hash::content_addr(&block),
        });
        Ok(WithConn {
            conn,
            value: progress,
        })
    })
    .await?
}

pub async fn sync_contracts<C, S>(
    conn: C,
    l2_block_number: Option<u64>,
    notify: watch::Sender<()>,
    stream: S,
) -> InternalResult<()>
where
    C: BorrowMut<rusqlite::Connection> + Send + 'static,
    S: Stream<Item = InternalResult<Contract>>,
{
    let mut l2_block_number = match l2_block_number {
        Some(l2_block_number) => l2_block_number.saturating_add(1),
        None => 0,
    };

    stream
        .try_fold(conn, move |conn, contract| {
            let this_l2_block_number = l2_block_number;
            l2_block_number += 1;
            write_contract(conn, contract, this_l2_block_number, notify.clone()).map_err(Into::into)
        })
        .await?;
    Ok(())
}

pub async fn sync_blocks<C, S>(
    conn: C,
    last_block_number: Option<u64>,
    notify: watch::Sender<()>,
    stream: S,
) -> InternalResult<()>
where
    C: BorrowMut<rusqlite::Connection> + Send + 'static,
    S: Stream<Item = InternalResult<Block>>,
{
    let mut block_number = match last_block_number {
        Some(last_block_number) => last_block_number.saturating_add(1),
        None => 0,
    };

    stream
        .try_fold(conn, move |conn, block| {
            let sequential_block = block.number == block_number;
            block_number = block.number.saturating_add(1);
            let notify = notify.clone();
            async move {
                if !sequential_block {
                    return Err(
                        RecoverableError::NonSequentialBlock(block_number, block.number).into(),
                    );
                }
                let conn = write_block(conn, block).await?;

                // Best effort to notify of new block
                let _ = notify.send(());
                Ok(conn)
            }
        })
        .await?;
    Ok(())
}

async fn write_contract<C>(
    mut conn: C,
    contract: Contract,
    l2_block_number: u64,
    notify: watch::Sender<()>,
) -> crate::Result<C>
where
    C: BorrowMut<rusqlite::Connection> + Send + 'static,
{
    let conn = spawn_blocking::<_, rusqlite::Result<_>>(move || {
        let contract_hash = essential_hash::contract_addr::from_contract(&contract);
        let tx = conn.borrow_mut().transaction()?;
        essential_node_db::insert_contract(&tx, &contract, l2_block_number)?;
        essential_node_db::insert_contract_progress(&tx, l2_block_number, &contract_hash)?;
        tx.commit()?;
        Ok(conn)
    })
    .await??;
    // Best effort to notify of new contract
    let _ = notify.send(());
    Ok(conn)
}

async fn write_block<C>(mut conn: C, block: Block) -> crate::Result<C>
where
    C: BorrowMut<rusqlite::Connection> + Send + 'static,
{
    spawn_blocking(move || {
        let tx = conn.borrow_mut().transaction()?;
        essential_node_db::insert_block(&tx, &block)?;
        tx.commit()?;
        Ok(conn)
    })
    .await?
}

impl Default for ContractProgress {
    fn default() -> Self {
        Self {
            l2_block_number: Default::default(),
            last_contract: ContentAddress(Default::default()),
        }
    }
}

impl From<(u64, ContentAddress)> for ContractProgress {
    fn from((l2_block_number, last_contract): (u64, ContentAddress)) -> Self {
        Self {
            l2_block_number,
            last_contract,
        }
    }
}
