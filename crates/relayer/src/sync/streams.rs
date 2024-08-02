use std::marker::PhantomData;

use essential_types::{contract::Contract, Block};
use futures::{Stream, StreamExt, TryStreamExt};
use reqwest::{Client, Url};
use tokio_util::{
    bytes::{self, Buf},
    codec::{Decoder, FramedRead},
    io::StreamReader,
};

use crate::{
    error::{CriticalError, InternalError, InternalResult, RecoverableError},
    DataSyncError,
};

use super::{BlockProgress, ContractProgress};

const SERVER_PAGE_SIZE: u64 = 100;

pub async fn stream_contracts(
    url: &Url,
    client: &Client,
    progress: Option<ContractProgress>,
) -> InternalResult<impl Stream<Item = InternalResult<Contract>>> {
    let next_contract_num = match &progress {
        Some(p) => p.l2_block_number.saturating_add(1),
        None => 0,
    };
    let page = next_contract_num / SERVER_PAGE_SIZE;

    let num_skip = next_contract_num % SERVER_PAGE_SIZE;
    let num_skip: usize = num_skip.try_into().map_err(|_| CriticalError::Overflow)?;

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

    let stream = stream.skip(num_skip);

    Ok(stream)
}

pub async fn stream_blocks(
    url: &Url,
    client: &Client,
    progress: Option<BlockProgress>,
) -> InternalResult<impl Stream<Item = InternalResult<Block>>> {
    let next_block_num = match &progress {
        Some(p) => p.last_block_number.saturating_add(1),
        None => 0,
    };

    let mut url = url
        .join("/subscribe-blocks")
        .map_err(|_| CriticalError::UrlParse)?;
    url.query_pairs_mut()
        .append_pair("block", &next_block_num.to_string());
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

pub(crate) async fn check_for_contract_mismatch(
    url: &Url,
    client: &Client,
    progress: &Option<ContractProgress>,
) -> InternalResult<()> {
    let Some(progress) = progress else {
        return Ok(());
    };
    let page = progress.l2_block_number / SERVER_PAGE_SIZE;

    let index = progress.l2_block_number % SERVER_PAGE_SIZE;
    let index: usize = index.try_into().map_err(|_| CriticalError::Overflow)?;

    let mut url = url
        .join("/list-contracts")
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

    let contracts: Vec<Contract> = response.json().await.map_err(RecoverableError::from)?;
    Ok(check_contract_fork(index, &contracts, progress)?)
}

fn check_contract_fork(
    index: usize,
    contracts: &[Contract],
    progress: &ContractProgress,
) -> crate::Result<()> {
    match contracts.get(index) {
        Some(contract) => {
            let contract_hash = essential_hash::contract_addr::from_contract(contract);
            if contract_hash != progress.last_contract {
                return Err(CriticalError::DataSyncFailed(
                    DataSyncError::ContractMismatch(
                        progress.l2_block_number,
                        progress.last_contract.clone(),
                        Some(contract_hash),
                    ),
                ));
            }
        }
        None => {
            return Err(CriticalError::DataSyncFailed(
                DataSyncError::ContractMismatch(
                    progress.l2_block_number,
                    progress.last_contract.clone(),
                    None,
                ),
            ));
        }
    }

    Ok(())
}

pub(crate) async fn check_for_block_fork(
    url: &Url,
    client: &Client,
    progress: &Option<BlockProgress>,
) -> InternalResult<()> {
    let Some(progress) = progress else {
        return Ok(());
    };

    let mut url = url
        .join("/list-blocks")
        .map_err(|_| CriticalError::UrlParse)?;
    url.query_pairs_mut()
        .append_pair("block", &progress.last_block_number.to_string());

    let response = client
        .get(url)
        .send()
        .await
        .map_err(RecoverableError::from)?;
    if !response.status().is_success() {
        return Err(RecoverableError::BadServerResponse(response.status()).into());
    }

    let blocks: Vec<Block> = response.json().await.map_err(RecoverableError::from)?;
    Ok(check_block_fork(&blocks, progress)?)
}

fn check_block_fork(blocks: &[Block], progress: &BlockProgress) -> crate::Result<()> {
    match blocks.first() {
        Some(block) => {
            let block_hash = essential_hash::content_addr(block);
            if block_hash != progress.last_block_hash {
                return Err(CriticalError::DataSyncFailed(DataSyncError::Fork(
                    progress.last_block_number,
                    progress.last_block_hash.clone(),
                    Some(block_hash),
                )));
            }
        }
        None => {
            return Err(CriticalError::DataSyncFailed(DataSyncError::Fork(
                progress.last_block_number,
                progress.last_block_hash.clone(),
                None,
            )));
        }
    }

    Ok(())
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
