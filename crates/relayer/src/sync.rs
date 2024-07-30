use error::ContractSyncError;
use essential_node_db::call;
use essential_types::{contract::Contract, ContentAddress};
use futures::Stream;
use rusqlite::Connection;

mod error;
#[cfg(test)]
mod tests;

pub struct ContractProgress {
    pub logical_clock: u64,
    pub last_contract: ContentAddress,
}

pub struct BlockProgress {
    pub last_block_number: u64,
    pub last_block_hash: ContentAddress,
}

pub struct WithConn<T> {
    pub conn: Connection,
    pub value: T,
}

pub async fn get_contract_progress(conn: Connection) -> Result<WithConn<ContractProgress>, ContractSyncError> {
    call(conn, |conn| {
        todo!()
    }).await
}

async fn sync_contracts<S>(db: Connection, stream: S) -> Result<(), ContractSyncError>
where
    S: Stream<Item = Result<Contract, ()>>,
{
    todo!()
}
