use std::marker::PhantomData;

use essential_types::{contract::Contract, Block};
use futures::{Stream, StreamExt, TryStreamExt};
use reqwest::{Client, Url};
use tokio_util::{
    bytes::{self, Buf},
    codec::{Decoder, FramedRead},
    io::StreamReader,
};

use crate::error::{CriticalError, InternalError, InternalResult, RecoverableError};

use super::{BlockProgress, ContractProgress};

const SERVER_PAGE_SIZE: u64 = 100;

pub async fn stream_contracts(
    url: &Url,
    client: &Client,
    progress: &Option<ContractProgress>,
) -> InternalResult<impl Stream<Item = InternalResult<Contract>>> {
    let (page, index) = match progress {
        Some(p) => {
            let page = p.l2_block_number / SERVER_PAGE_SIZE;
            let index = p.l2_block_number % SERVER_PAGE_SIZE;
            let index: usize = index.try_into().map_err(|_| CriticalError::Overflow)?;
            (page, index)
        }
        None => (0, 0),
    };

    let mut url = url
        .join("/subscribe-contracts")
        .map_err(|_| CriticalError::UrlParse)?;
    url.query_pairs_mut().append_pair("page", &page.to_string());
    let response = client
        .get(url)
        .send()
        .await
        .map_err(RecoverableError::from)?;
    if !response.status().is_success() {
        return Err(RecoverableError::BadServerResponse(response.status()).into());
    }

    let stream = StreamReader::new(
        response
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e))),
    );
    let stream = FramedRead::new(stream, SseDecoder::<Contract>::new());

    let stream = stream.skip(index);

    Ok(stream)
}

pub async fn stream_blocks(
    url: &Url,
    client: &Client,
    progress: &Option<BlockProgress>,
) -> InternalResult<impl Stream<Item = InternalResult<Block>>> {
    let last_block_number = progress
        .as_ref()
        .map(|p| p.last_block_number)
        .unwrap_or_default();

    let mut url = url
        .join("/subscribe-blocks")
        .map_err(|_| CriticalError::UrlParse)?;
    url.query_pairs_mut()
        .append_pair("block", &last_block_number.to_string());
    let response = client
        .get(url)
        .send()
        .await
        .map_err(RecoverableError::from)?;
    if !response.status().is_success() {
        return Err(RecoverableError::BadServerResponse(response.status()).into());
    }

    let stream = StreamReader::new(
        response
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e))),
    );
    let stream = FramedRead::new(stream, SseDecoder::<Block>::new());

    Ok(stream)
}

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
        let end = buf
            .iter()
            .zip(buf.iter().skip(1))
            .position(|(&a, &b)| a == b'\n' && b == b'\n');

        match end {
            Some(end) => {
                let Ok(s) = std::str::from_utf8(&buf[..end]) else {
                    buf.advance(end + 2);
                    return Ok(None);
                };
                let s = s.trim_start_matches("data: ").trim();
                let data = serde_json::from_str::<T>(s);
                let r = match data {
                    Ok(data) => Ok(Some(data)),
                    Err(_) => {
                        // Keep-alive
                        if s == ":" {
                            Ok(None)
                        } else {
                            Err(RecoverableError::StreamError(s.to_string()).into())
                        }
                    }
                };
                buf.advance(end + 2);
                r
            }
            None => Ok(None),
        }
    }
}
