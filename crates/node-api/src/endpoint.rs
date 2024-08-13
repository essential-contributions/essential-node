//! Provides a small module for each endpoint with associated `PATH` and `handler`.

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use core::ops::Range;
use essential_node::db;
use essential_types::{
    contract::Contract, convert::word_from_bytes, predicate::Predicate, Block, ContentAddress,
    Value, Word,
};
use thiserror::Error;

/// Any endpoint error that might occur.
#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to decode from hex string: {0}")]
    HexDecode(#[from] hex::FromHexError),
    #[error("DB query failed: {0}")]
    ConnPoolQuery(#[from] db::AcquireThenQueryError),
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;
        match self {
            Error::ConnPoolQuery(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
            Error::HexDecode(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        }
    }
}

/// The return a health check response.
pub mod health_check {
    pub const PATH: &str = "/";
    pub async fn handler() {}
}

/// The `get-predicate` get endpoint.
///
/// Takes a predicate content address (encoded as hex) as a path parameter.
pub mod get_contract {
    use super::*;
    pub const PATH: &str = "/get-contract/:contract-ca";
    pub async fn handler(
        State(conn_pool): State<db::ConnectionPool>,
        Path(contract_ca): Path<String>,
    ) -> Result<Json<Option<Contract>>, Error> {
        let ca: ContentAddress = contract_ca.parse()?;
        let contract = conn_pool.get_contract(ca).await?;
        Ok(Json(contract))
    }
}

/// The `get-predicate` get endpoint.
///
/// Takes a predicate content address (encoded as hex) as a path parameter.
pub mod get_predicate {
    use super::*;
    pub const PATH: &str = "/get-predicate/:predicate-ca";
    pub async fn handler(
        State(conn_pool): State<db::ConnectionPool>,
        Path(predicate_ca): Path<String>,
    ) -> Result<Json<Option<Predicate>>, Error> {
        let ca = predicate_ca.parse()?;
        let predicate = conn_pool.get_predicate(ca).await?;
        Ok(Json(predicate))
    }
}

/// The `list-blocks` get endpoint.
///
/// Takes a range of L2 blocks as a parameter.
pub mod list_blocks {
    use super::*;
    pub const PATH: &str = "/list-blocks";
    pub async fn handler(
        State(conn_pool): State<db::ConnectionPool>,
        Query(block_range): Query<Range<u64>>,
    ) -> Result<Json<Vec<Block>>, Error> {
        let blocks = conn_pool.list_blocks(block_range).await?;
        Ok(Json(blocks))
    }
}

/// The `list-contracts` get endpoint.
///
/// Takes a range of L2 blocks as a parameter.
pub mod list_contracts {
    use super::*;
    pub const PATH: &str = "/list-contracts";
    pub async fn handler(
        State(conn_pool): State<db::ConnectionPool>,
        Query(block_range): Query<Range<u64>>,
    ) -> Result<Json<Vec<(u64, Vec<Contract>)>>, Error> {
        let contracts = conn_pool.list_contracts(block_range).await?;
        Ok(Json(contracts))
    }
}

/// The `query-state` get endpoint.
///
/// Takes a contract content address and a byte array key as path parameters,
/// both encoded as hex.
pub mod query_state {
    use super::*;
    pub const PATH: &str = "/query-state/:contract-ca/:key";
    pub async fn handler(
        State(conn_pool): State<db::ConnectionPool>,
        Path((contract_ca, key)): Path<(String, String)>,
    ) -> Result<Json<Option<Value>>, Error> {
        let contract_ca: ContentAddress = contract_ca.parse()?;
        let key: Vec<u8> = hex::decode(key)?;
        let key = key_words_from_bytes(&key);
        let value = conn_pool.query_state(contract_ca, key).await?;
        Ok(Json(value))
    }
}

fn key_words_from_bytes(key: &[u8]) -> Vec<Word> {
    key.chunks_exact(core::mem::size_of::<Word>())
        .map(|chunk| word_from_bytes(chunk.try_into().expect("safe due to chunk size")))
        .collect::<Vec<_>>()
}
