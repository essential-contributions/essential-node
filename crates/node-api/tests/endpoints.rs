use std::time::Duration;

use essential_node::{self as node, test_utils::register_contracts_block, BlockTx};
use essential_node_api as node_api;
use essential_node_types::BigBang;
use essential_types::{convert::bytes_from_word, Block, Value};
use futures::{StreamExt, TryStreamExt};
use tokio_util::{
    bytes::{self, Buf},
    codec::FramedRead,
    io::StreamReader,
};
use util::{
    client, get_url, init_tracing_subscriber, reqwest_get, state_db_only, test_conn_pool,
    with_test_server,
};

mod util;

#[tokio::test]
async fn test_health_check() {
    #[cfg(feature = "tracing")]
    init_tracing_subscriber();

    let db = test_conn_pool();
    with_test_server(state_db_only(db), |port| async move {
        let response = reqwest_get(port, node_api::endpoint::health_check::PATH).await;
        assert!(response.status().is_success());
    })
    .await;
}

#[tokio::test]
async fn test_query_state() {
    #[cfg(feature = "tracing")]
    init_tracing_subscriber();

    let db = test_conn_pool();

    // The test state.
    let seed = 11;
    let block_n = 100;
    let block_ts = Duration::from_secs(block_n as _);
    let contract = node::test_utils::test_contract(seed);

    // Make some randomish keys and values.
    let mut keys = vec![];
    let mut values = vec![];
    for i in 0i64..256 {
        let key = vec![(i + 1) * 5; 1 + (i as usize * 103) % 128];
        let value = vec![(i + 1) * 7; 1 + (i as usize * 391) % 128];
        keys.push(key);
        values.push(value);
    }

    // Insert a contract to own the state.
    let registry = BigBang::default().contract_registry;
    let block = register_contracts_block(registry, Some(&contract), block_n, block_ts).unwrap();
    let block_ca = db.insert_block(block.into()).await.unwrap();
    db.finalize_block(block_ca).await.unwrap();

    let contract_ca = essential_hash::content_addr(&contract);

    // Insert the state entries.
    for (k, v) in keys.iter().zip(&values) {
        let ca = contract_ca.clone();
        let (key, value) = (k.clone(), v.clone());
        db.update_state(ca, key, value).await.unwrap();
    }

    // Query each of the keys and check they match what we expect.
    with_test_server(state_db_only(db), |port| async move {
        for (k, v) in keys.iter().zip(&values) {
            let key_bytes: Vec<_> = k.iter().copied().flat_map(bytes_from_word).collect();
            let key = hex::encode(&key_bytes);
            let response = reqwest_get(port, &format!("/query-state/{contract_ca}/{key}")).await;
            assert!(response.status().is_success());
            let response_value = response.json::<Option<Value>>().await.unwrap();
            assert_eq!(Some(v), response_value.as_ref());
        }
    })
    .await;
}

#[tokio::test]
async fn test_list_blocks() {
    #[cfg(feature = "tracing")]
    init_tracing_subscriber();

    let db = test_conn_pool();

    // Create some test blocks.
    let n_blocks = 100;
    let (blocks, _) = node::test_utils::test_blocks(n_blocks);

    // Insert them into the node's DB.
    for block in &blocks {
        db.insert_block(std::sync::Arc::new(block.clone()))
            .await
            .unwrap();
    }

    // Fetch all blocks.
    let fetched_blocks = with_test_server(state_db_only(db), |port| async move {
        let response = client()
            .get(get_url(
                port,
                &format!("/list-blocks?start={}&end={}", 0, n_blocks),
            ))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        response.json::<Vec<Block>>().await.unwrap()
    })
    .await;

    assert_eq!(blocks, fetched_blocks);
}

#[tokio::test]
async fn test_subscribe_blocks() {
    #[cfg(feature = "tracing")]
    init_tracing_subscriber();

    let db = test_conn_pool();

    // The test blocks.
    let (blocks, _) = node::test_utils::test_blocks(1000);

    // A fn for notifying of new blocks.
    let block_tx = BlockTx::new();
    let block_rx = block_tx.new_listener();

    // Write the first 10 blocks to the DB. We'll write the rest later.
    for block in &blocks[..10] {
        let block = std::sync::Arc::new(block.clone());
        db.insert_block(block).await.unwrap();
    }

    // Start a test server and subscribe to blocks.
    let blocks2 = blocks.clone();
    let state = node_api::State {
        conn_pool: db.clone(),
        new_block: Some(block_rx),
    };
    let server = with_test_server(state, |port| async move {
        let response = reqwest_get(port, "/subscribe-blocks?start_block=0").await;

        // Create the stream from the response.
        let bytes_stream = StreamReader::new(
            response
                .bytes_stream()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e))),
        );
        let mut frame_stream = FramedRead::new(bytes_stream, SseDecoder::<Block>::new());

        // There should always be 10 blocks available to begin as we wrote those first.
        let fetched_blocks: Vec<_> = frame_stream
            .by_ref()
            .take(10)
            .map(Result::unwrap)
            .collect()
            .await;

        assert_eq!(&blocks2[..10], &fetched_blocks);

        // The stream should yield the remaining blocks and then complete after the
        // `new_block_tx` drops.
        let fetched_blocks: Vec<_> = frame_stream.map(Result::unwrap).collect().await;
        assert_eq!(&blocks2[10..], &fetched_blocks);
    });

    // Write the remaining blocks asynchronously, notifying on each new block.
    let blocks_remaining = blocks[10..].to_vec();
    let write_remaining_blocks = tokio::spawn(async move {
        for block in blocks_remaining {
            db.insert_block(block.into()).await.unwrap();
            block_tx.notify();
        }
        // After writing, drop the new block tx, closing the stream.
        std::mem::drop(block_tx);
    });

    let ((), res) = tokio::join!(server, write_remaining_blocks);
    res.unwrap();
}

// -------------------------------------------------------------------
// TODO: Following copied from `relayer/src/sync/streams` to decode SSE.
//       Move into it's own crate? Or use `tokio_sse_codec` crate?

/// Decoder for the server SSE stream.
struct SseDecoder<T>(core::marker::PhantomData<T>);

impl<T> SseDecoder<T> {
    fn new() -> Self {
        Self(core::marker::PhantomData)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("SSE decode error")]
pub enum SseDecodeError {
    #[error("an I/O error occurred: {0}")]
    Io(#[from] std::io::Error),
}

impl<T> tokio_util::codec::Decoder for SseDecoder<T>
where
    T: serde::de::DeserializeOwned,
{
    type Item = T;
    type Error = SseDecodeError;

    fn decode(&mut self, buf: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // SSE streams are separated by two new lines.
        let end = buf
            .iter()
            .zip(buf.iter().skip(1))
            .position(|(&a, &b)| a == b'\n' && b == b'\n');

        match end {
            Some(end) => {
                // Parse the data from the stream as utf8.
                let Ok(s) = std::str::from_utf8(&buf[..end]) else {
                    // If this fails we still have to advance the buffer.
                    buf.advance(end + 2);

                    // This will skip this bad data.
                    return Ok(None);
                };

                // SSE streams have a `data:` prefix.
                let s = s.trim_start_matches("data: ").trim();

                // Parse the data from the stream.
                let data = serde_json::from_str::<T>(s);

                let r = match data {
                    // Success data found.
                    Ok(data) => Ok(Some(data)),
                    // Error parsing the data.
                    Err(_) => {
                        // Check if it's just a Keep-alive signal.
                        if s == ":" {
                            Ok(None)
                        } else {
                            // This is a stream error.
                            panic!("stream error: {s}");
                        }
                    }
                };

                // Advance the buffer.
                buf.advance(end + 2);
                r
            }
            // Need more data
            None => Ok(None),
        }
    }
}
