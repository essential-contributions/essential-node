use essential_types::contract::Contract;
use futures::{Stream, StreamExt, TryStreamExt};
use reqwest::{Client, Url};
use tokio_util::{
    bytes::{self, Buf},
    codec::{Decoder, FramedRead},
    io::StreamReader,
};

use crate::Error;

use super::ContractProgress;

const SERVER_PAGE_SIZE: u64 = 100;

pub async fn stream_contracts(
    url: &Url,
    client: &Client,
    progress: Option<ContractProgress>,
) -> Result<impl Stream<Item = Result<Contract, Error>>, Error> {
    let num_contracts_received = progress.as_ref().map(|p| p.logical_clock).unwrap_or(0);
    let mut page = num_contracts_received / SERVER_PAGE_SIZE;

    // If the last contract is the last contract on the page, we need to go back one page.
    if num_contracts_received % SERVER_PAGE_SIZE == 0 {
        page = page.saturating_sub(1);
    }

    let mut url = url
        .join("/subscribe-contracts")
        .map_err(|_| Error::UrlParse)?;
    url.query_pairs_mut().append_pair("page", &page.to_string());
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(Error::BadServerResponse(response.status()));
    }

    let stream = StreamReader::new(
        response
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e))),
    );
    let stream = FramedRead::new(stream, ContractDecoder {});

    // FIXME: This could be infinite if the last contract is not found.
    // Need to end if a page has passed without finding the last contract.
    let stream = stream.skip_while(move |contract| {
        let Ok(contract) = contract else {
            return futures::future::ready(false);
        };

        let r = match &progress {
            Some(ContractProgress { last_contract, .. }) => {
                essential_hash::contract_addr::from_contract(contract) != *last_contract
            }
            None => false,
        };
        futures::future::ready(r)
    });

    Ok(stream)
}

struct ContractDecoder {}

impl Decoder for ContractDecoder {
    type Item = Contract;
    type Error = Error;

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
                let contract = serde_json::from_str::<Contract>(s);
                buf.advance(end + 2);
                let Ok(contract) = contract else {
                    // TODO: Handle incoming errors in the stream.
                    return Ok(None);
                };
                Ok(Some(contract))
            }
            None => Ok(None),
        }
    }
}
