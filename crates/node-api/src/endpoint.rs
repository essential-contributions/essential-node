//! Provides a small module for each endpoint with associated `PATH` and `handler`.

use axum::{
    extract::{Path, Query, State},
    response::{
        sse::{self, Sse},
        IntoResponse,
    },
    Json,
};
use essential_node::db;
use essential_types::{
    contract::Contract, convert::word_from_bytes, predicate::Predicate, Block, ContentAddress,
    Value, Word,
};
use futures::{Stream, StreamExt};
use serde::Deserialize;
use thiserror::Error;

/// A range in blocks, used for the `list-blocks` and `list-contracts` endpoints.
///
/// The range is non-inclusive of the `end`, i.e. it is equivalent to `start..end`.
#[derive(Deserialize)]
pub struct BlockRange {
    pub start: u64,
    pub end: u64,
}

/// Any endpoint error that might occur.
#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to decode from hex string: {0}")]
    HexDecode(#[from] hex::FromHexError),
    #[error("DB query failed: {0}")]
    ConnPoolQuery(#[from] db::AcquireThenQueryError),
}

/// An error produced by a subscription endpoint stream.
#[derive(Debug, Error)]
pub enum SubscriptionError {
    /// An axum error occurred.
    #[error("an axum error occurred: {0}")]
    Axum(#[from] axum::Error),
    /// A DB query failure occurred.
    #[error("DB query failed: {0}")]
    Query(#[from] db::QueryError),
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
/// Takes a contract content address (encoded as hex) as a path parameter.
pub mod get_contract {
    use super::*;
    pub const PATH: &str = "/get-contract/:contract-ca";
    pub async fn handler(
        State(state): State<crate::State>,
        Path(contract_ca): Path<String>,
    ) -> Result<Json<Option<Contract>>, Error> {
        let ca: ContentAddress = contract_ca.parse()?;
        let contract = state.conn_pool.get_contract(ca).await?;
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
        State(state): State<crate::State>,
        Path(predicate_ca): Path<String>,
    ) -> Result<Json<Option<Predicate>>, Error> {
        let ca = predicate_ca.parse()?;
        let predicate = state.conn_pool.get_predicate(ca).await?;
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
        State(state): State<crate::State>,
        Query(block_range): Query<BlockRange>,
    ) -> Result<Json<Vec<Block>>, Error> {
        let blocks = state
            .conn_pool
            .list_blocks(block_range.start..block_range.end)
            .await?;
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
        State(state): State<crate::State>,
        Query(block_range): Query<BlockRange>,
    ) -> Result<Json<Vec<(u64, Vec<Contract>)>>, Error> {
        let contracts = state
            .conn_pool
            .list_contracts(block_range.start..block_range.end)
            .await?;
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
        State(state): State<crate::State>,
        Path((contract_ca, key)): Path<(String, String)>,
    ) -> Result<Json<Option<Value>>, Error> {
        let contract_ca: ContentAddress = contract_ca.parse()?;
        let key: Vec<u8> = hex::decode(key)?;
        let key = key_words_from_bytes(&key);
        let value = state.conn_pool.query_state(contract_ca, key).await?;
        Ok(Json(value))
    }
}

/// The `subscribe-blocks` get endpoint.
///
/// Produces an event for every block starting from the given block number.
pub mod subscribe_blocks {
    use super::*;
    pub const PATH: &str = "/subscribe-blocks";
    pub async fn handler(
        State(state): State<crate::State>,
        Query(start_block): Query<u64>,
    ) -> Sse<impl Stream<Item = Result<sse::Event, SubscriptionError>>> {
        // Create the `await_new_block` fn.
        let new_block = state.new_block.clone();
        let await_new_block = move || {
            let new_block = new_block.clone();
            async move {
                match new_block {
                    None => None,
                    Some(mut rx) => rx.changed().await.ok(),
                }
            }
        };

        // The block stream.
        let blocks = state
            .conn_pool
            .subscribe_blocks(start_block, await_new_block);

        // Map the stream of blocks to SSE events.
        let sse_events = blocks.map(|res| {
            let block = res?;
            let event = sse::Event::default().json_data(block)?;
            Ok(event)
        });

        Sse::new(sse_events).keep_alive(sse::KeepAlive::default())
    }
}

fn key_words_from_bytes(key: &[u8]) -> Vec<Word> {
    key.chunks_exact(core::mem::size_of::<Word>())
        .map(|chunk| word_from_bytes(chunk.try_into().expect("safe due to chunk size")))
        .collect::<Vec<_>>()
}
