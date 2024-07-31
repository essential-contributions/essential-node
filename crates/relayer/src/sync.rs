use essential_types::{contract::Contract, ContentAddress};
use futures::stream::TryStreamExt;
use futures::Stream;
use rusqlite::Connection;
use tokio::task::spawn_blocking;

pub use contract_stream::stream_contracts;

use crate::Error;

mod contract_stream;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContractProgress {
    pub logical_clock: u64,
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

pub async fn sync_contracts<S>(
    conn: Connection,
    mut logical_clock: u64,
    stream: S,
) -> Result<(), Error>
where
    S: Stream<Item = Result<Contract, Error>>,
{
    stream
        .try_fold(conn, move |conn, contract| {
            logical_clock += 1;
            write_contract(conn, contract, logical_clock)
        })
        .await?;
    Ok(())
}

async fn write_contract(
    mut conn: Connection,
    contract: Contract,
    logical_clock: u64,
) -> Result<rusqlite::Connection, Error> {
    spawn_blocking(move || {
        let contract_hash = essential_hash::contract_addr::from_contract(&contract);
        let tx = conn.transaction()?;
        essential_node_db::insert_contract(&tx, &contract, logical_clock)?;
        essential_node_db::insert_contract_progress(&tx, logical_clock, &contract_hash)?;
        tx.commit()?;
        Ok(conn)
    })
    .await?
}

impl Default for ContractProgress {
    fn default() -> Self {
        Self {
            logical_clock: Default::default(),
            last_contract: ContentAddress(Default::default()),
        }
    }
}

impl From<(u64, ContentAddress)> for ContractProgress {
    fn from((logical_clock, last_contract): (u64, ContentAddress)) -> Self {
        Self {
            logical_clock,
            last_contract,
        }
    }
}
