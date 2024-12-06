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
use essential_node_types::{block_notify::BlockRx, Block};
use essential_types::{convert::word_from_bytes, ContentAddress, Value, Word};
use futures::{Stream, StreamExt};
use serde::Deserialize;
use thiserror::Error;

/// A range in blocks, used for the `list-blocks` and `list-contracts` endpoints.
///
/// The range is non-inclusive of the `end`, i.e. it is equivalent to `start..end`.
#[derive(Deserialize)]
pub struct BlockRange {
    /// Start of the range.
    pub start: Word,
    /// The end of the range (exclusive).
    pub end: Word,
}

/// Type to deserialize a block number query parameter.
#[derive(Deserialize)]
pub struct StartBlock {
    /// The block number to start from.
    pub start_block: Word,
}

/// Any endpoint error that might occur.
#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to decode from hex string: {0}")]
    HexDecode(#[from] hex::FromHexError),
    #[error("DB query failed: {0}")]
    ConnPoolQuery(#[from] db::pool::AcquireThenQueryError),
    #[error(
        "Invalid query parameter for /query-state: {0}. {}",
        query_state::HELP_MSG
    )]
    InvalidQueryParameters(query_state::QueryStateParams),
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

/// Provides an [`db::AwaitNewBlock`] implementation for the API.
struct AwaitNewBlock(Option<BlockRx>);

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;
        match self {
            Error::ConnPoolQuery(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
            e @ Error::HexDecode(_) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            e @ Error::InvalidQueryParameters(_) => {
                (StatusCode::BAD_REQUEST, e.to_string()).into_response()
            }
        }
    }
}

impl db::AwaitNewBlock for AwaitNewBlock {
    async fn await_new_block(&mut self) -> Option<()> {
        match self.0 {
            None => None,
            Some(ref mut rx) => rx.changed().await.ok(),
        }
    }
}

/// The return a health check response.
pub mod health_check {
    pub const PATH: &str = "/";
    pub async fn handler() {}
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

/// The `query-state` get endpoint.
///
/// Takes a contract content address and a byte array key as path parameters,
/// both encoded as hex.
pub mod query_state {
    use std::fmt::Display;

    use serde::Serialize;

    use super::*;

    pub const HELP_MSG: &str = r#"
The query parameters must be empty or one of the following combinations:
    - block_inclusive
    - block_exclusive
    - block_inclusive, solution_inclusive
    - block_inclusive, solution_exclusive
"#;

    #[derive(Deserialize, Serialize, Default, Debug)]
    pub struct QueryStateParams {
        pub block_inclusive: Option<Word>,
        pub block_exclusive: Option<Word>,
        pub solution_inclusive: Option<u64>,
        pub solution_exclusive: Option<u64>,
    }

    impl Display for QueryStateParams {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "block_inclusive: {:?}, block_exclusive: {:?}, solution_inclusive: {:?}, solution_exclusive: {:?}",
                self.block_inclusive, self.block_exclusive, self.solution_inclusive, self.solution_exclusive
            )
        }
    }

    pub const PATH: &str = "/query-state/:contract-ca/:key";
    pub async fn handler(
        State(state): State<crate::State>,
        Path((contract_ca, key)): Path<(String, String)>,
        Query(params): Query<QueryStateParams>,
    ) -> Result<Json<Option<Value>>, Error> {
        let contract_ca: ContentAddress = contract_ca.parse()?;
        let key: Vec<u8> = hex::decode(key)?;
        let key = key_words_from_bytes(&key);
        // TODO: When state is compacted and blocks are discarded, this query should
        // fall back to querying compacted state.

        // TODO: When blocks aren't immediately finalized, this query will need to
        // either take a block address or use a fork choice rule to determine the
        // latest state to return. It's possible this query won't make much sense
        // at that point.

        let value = match params {
            QueryStateParams {
                block_inclusive: Some(block),
                block_exclusive: None,
                solution_inclusive: None,
                solution_exclusive: None,
            } => {
                state
                    .conn_pool
                    .query_state_finalized_inclusive_block(contract_ca, key, block)
                    .await?
            }
            QueryStateParams {
                block_inclusive: None,
                block_exclusive: Some(block),
                solution_inclusive: None,
                solution_exclusive: None,
            } => {
                state
                    .conn_pool
                    .query_state_finalized_exclusive_block(contract_ca, key, block)
                    .await?
            }
            QueryStateParams {
                block_inclusive: Some(block),
                block_exclusive: None,
                solution_inclusive: Some(solution_ix),
                solution_exclusive: None,
            } => {
                state
                    .conn_pool
                    .query_state_finalized_inclusive_solution_set(
                        contract_ca,
                        key,
                        block,
                        solution_ix,
                    )
                    .await?
            }
            QueryStateParams {
                block_inclusive: Some(block),
                block_exclusive: None,
                solution_inclusive: None,
                solution_exclusive: Some(solution_ix),
            } => {
                state
                    .conn_pool
                    .query_state_finalized_exclusive_solution_set(
                        contract_ca,
                        key,
                        block,
                        solution_ix,
                    )
                    .await?
            }
            QueryStateParams {
                block_inclusive: None,
                block_exclusive: None,
                solution_inclusive: None,
                solution_exclusive: None,
            } => {
                state
                    .conn_pool
                    .query_latest_finalized_block(contract_ca, key)
                    .await?
            }
            _ => return Err(Error::InvalidQueryParameters(params)),
        };
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
        Query(StartBlock { start_block }): Query<StartBlock>,
    ) -> Sse<impl Stream<Item = Result<sse::Event, SubscriptionError>>> {
        // The block stream.
        let new_block = AwaitNewBlock(state.new_block.clone());
        let blocks = state.conn_pool.subscribe_blocks(start_block, new_block);

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
