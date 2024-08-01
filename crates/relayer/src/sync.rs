use essential_types::Block;
use essential_types::{contract::Contract, ContentAddress};
use futures::stream::TryStreamExt;
use futures::Stream;
use rusqlite::Connection;
use tokio::sync::watch;
use tokio::task::spawn_blocking;

pub(crate) use streams::check_for_block_fork;
pub(crate) use streams::check_for_contract_mismatch;
pub(crate) use streams::stream_blocks;
pub(crate) use streams::stream_contracts;

use crate::Error;

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

pub struct WithConn<T> {
    pub conn: Connection,
    pub value: T,
}

pub async fn get_contract_progress(
    conn: Connection,
) -> Result<WithConn<Option<ContractProgress>>, Error> {
    tokio::task::spawn_blocking(move || {
        let progress = essential_node_db::get_contract_progress(&conn)?;
        Ok(WithConn {
            conn,
            value: progress.map(Into::into),
        })
    })
    .await?
}

pub async fn get_block_progress(
    mut conn: Connection,
) -> Result<WithConn<Option<BlockProgress>>, Error> {
    tokio::task::spawn_blocking(move || {
        let tx = conn.transaction()?;
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

pub async fn sync_contracts<S>(
    conn: Connection,
    l2_block_number: Option<u64>,
    stream: S,
) -> Result<(), Error>
where
    S: Stream<Item = Result<Contract, Error>>,
{
    let mut l2_block_number = match l2_block_number {
        Some(l2_block_number) => l2_block_number.saturating_add(1),
        None => 0,
    };

    stream
        .try_fold(conn, move |conn, contract| {
            let this_l2_block_number = l2_block_number;
            l2_block_number += 1;
            write_contract(conn, contract, this_l2_block_number)
        })
        .await?;
    Ok(())
}

pub async fn sync_blocks<S>(
    conn: Connection,
    last_block_number: Option<u64>,
    notify: watch::Sender<()>,
    stream: S,
) -> Result<(), Error>
where
    S: Stream<Item = Result<Block, Error>>,
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
                    return Err(Error::NonSequentialBlock(block_number, block.number));
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

async fn write_contract(
    mut conn: Connection,
    contract: Contract,
    l2_block_number: u64,
) -> Result<rusqlite::Connection, Error> {
    spawn_blocking(move || {
        let contract_hash = essential_hash::contract_addr::from_contract(&contract);
        let tx = conn.transaction()?;
        essential_node_db::insert_contract(&tx, &contract, l2_block_number)?;
        essential_node_db::insert_contract_progress(&tx, l2_block_number, &contract_hash)?;
        tx.commit()?;
        Ok(conn)
    })
    .await?
}

async fn write_block(mut conn: Connection, block: Block) -> Result<rusqlite::Connection, Error> {
    spawn_blocking(move || {
        let tx = conn.transaction()?;
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
