//! Server streams for syncing data.
//!
//! Most of this module will get thrown away once we start syncing
//! from a real L1 chain.
//! Best efforts have been made to make this module as correct as possible
//! but that needs to be balanced with over engineering temporary code.

use super::BlockProgress;
use crate::error::{CriticalError, InternalError, InternalResult, RecoverableError};
use essential_types::Block;
use futures::{Stream, TryStreamExt};
use reqwest::{Client, Url};
use std::marker::PhantomData;
use tokio_util::{
    bytes::{self, Buf},
    codec::{Decoder, FramedRead},
    io::StreamReader,
};

/// Create the stream of blocks from the node endpoint.
pub(crate) async fn stream_blocks(
    url: &Url,
    client: &Client,
    progress: &Option<BlockProgress>,
) -> InternalResult<impl Stream<Item = InternalResult<Block>>> {
    // Get the last block number that was synced.
    let last_block_number = progress
        .as_ref()
        .map(|p| p.last_block_number)
        .unwrap_or_default();

    // Create the subscription to the node's blocks stream.
    let mut url = url
        .join("/subscribe-blocks")
        .map_err(|_| CriticalError::UrlParse)?;

    // Start from the last block number.
    url.query_pairs_mut()
        .append_pair("start_block", &last_block_number.to_string());

    // Send the request to the node.
    let response = client
        .get(url)
        .send()
        .await
        .map_err(RecoverableError::from)?;

    // Check if the node returned a bad response.
    if !response.status().is_success() {
        return Err(RecoverableError::BadServerResponse(response.status()).into());
    }

    // Create the stream from the response.
    let stream = StreamReader::new(
        response
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e))),
    );

    // Decode the stream from the node.
    let stream = FramedRead::new(stream, SseDecoder::<Block>::new());

    Ok(stream)
}

/// Decoder for the node SSE stream.
struct SseDecoder<T>(PhantomData<T>);

impl<T> SseDecoder<T> {
    fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Decoder for SseDecoder<T>
where
    T: serde::de::DeserializeOwned,
{
    type Item = T;
    type Error = InternalError;

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
                            Err(RecoverableError::StreamError(s.to_string()).into())
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
