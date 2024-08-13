// #![allow(dead_code)]

use crate::{
    error::{self, CriticalError, InternalError, RecoverableError},
    handle::Handle,
};
use essential_node_db::{get_state_progress, list_blocks, update_state, update_state_progress};
use essential_types::{solution::Mutation, Block, ContentAddress};
use futures::stream::{StreamExt, TryStreamExt};
use std::{borrow::BorrowMut, future::Future};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

#[cfg(test)]
mod tests;

pub trait GetConn {
    /// The error type.
    type Error;
    /// The connection type.
    type Connection: BorrowMut<rusqlite::Connection> + Send + 'static;

    /// Get a connection to the database.
    fn get(
        &self,
    ) -> impl Future<Output = std::result::Result<Self::Connection, Self::Error>> + Send;
}

pub struct Mutations<I>
where
    I: IntoIterator<Item = Mutation>,
{
    pub contract_address: ContentAddress,
    pub mutations: I,
}

pub async fn block_stream<C>(
    get_conn: C,
    block_rx: watch::Receiver<()>,
) -> Result<Handle<CriticalError>, CriticalError>
where
    C: GetConn + Clone + Send + 'static,
    CriticalError: From<C::Error>,
{
    let (shutdown, stream_close) = watch::channel(());

    let jh = tokio::spawn(async move {
        loop {
            let rx = WatchStream::new(block_rx.clone());
            let mut stream_close = stream_close.clone();
            let close = async move {
                let _ = stream_close.changed().await;
            };
            let conn = get_conn.get().await?;

            let r = rx
                .take_until(close)
                .map(Ok)
                .try_fold(conn, |conn, _| process_block(conn))
                .await;

            match r {
                // Stream has ended, return from the task
                Ok(_) => return Ok(()),
                Err(e) => {
                    // Return error if it's critical or
                    // continue if it's recoverable
                    handle_error(e)?;
                }
            }
        }
    });

    Ok(Handle::new(jh, shutdown))
}

async fn process_block<C>(conn: C) -> Result<C, InternalError>
where
    C: BorrowMut<rusqlite::Connection> + Send + 'static,
{
    let (conn, progress) = get_last_progress(conn).await?;

    let (conn, block) = get_next_block(conn, progress).await?;
    let block_hash = essential_hash::content_addr(&block);

    let mutations = block.solutions.into_iter().flat_map(|solution| {
        solution.data.into_iter().map(|data| Mutations {
            contract_address: data.predicate_to_solve.contract,
            mutations: data.state_mutations,
        })
    });

    Ok(update_state_in_db(conn, mutations, block.number, &block_hash).await?)
}

async fn get_last_progress<C>(
    conn: C,
) -> Result<(C, Option<(u64, ContentAddress)>), RecoverableError>
where
    C: BorrowMut<rusqlite::Connection> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let r = get_state_progress(conn.borrow()).map_err(|_err| RecoverableError::LastProgress)?;
        Ok((conn, r))
    })
    .await?
}

async fn get_next_block<C>(
    conn: C,
    progress: Option<(u64, ContentAddress)>,
) -> Result<(C, Block), InternalError>
where
    C: BorrowMut<rusqlite::Connection> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let range = progress.as_ref().map_or(0..1, |(block_num, _)| {
            *block_num..block_num.saturating_add(2) // TODO: hack
        });

        let blocks = list_blocks(conn.borrow(), range).map_err(RecoverableError::ReadState)?;

        let block = match progress {
            Some((number, hash)) => {
                let mut iter = blocks.into_iter();
                let previous_block = iter.next().ok_or(CriticalError::Fork)?;
                let current_block = iter.next().ok_or(RecoverableError::BlockNotFound(number))?;
                if essential_hash::content_addr(&previous_block) != hash {
                    return Err(CriticalError::Fork.into());
                }
                current_block
            }
            None => blocks
                .into_iter()
                .next()
                .ok_or(RecoverableError::BlockNotFound(0))?,
        };

        Ok((conn, block))
    })
    .await
    .map_err(RecoverableError::Join)?
}

#[cfg_attr(feature = "tracing", tracing::instrument("state_progress", skip_all))]
async fn update_state_in_db<C, S, I>(
    mut conn: C,
    mutations: I,
    block_number: u64,
    block_hash: &ContentAddress,
) -> Result<C, RecoverableError>
where
    C: BorrowMut<rusqlite::Connection> + Send + 'static,
    S: IntoIterator<Item = Mutation>,
    I: IntoIterator<Item = Mutations<S>> + Send + 'static,
{
    let f = {
        let tx = conn.borrow_mut().transaction()?;

        for mutation in mutations {
            for m in mutation.mutations {
                update_state(&tx, &mutation.contract_address, &m.key, &m.value)
                    .map_err(RecoverableError::WriteState)?;
            }
        }

        update_state_progress(&tx, block_number, block_hash)
            .map_err(RecoverableError::WriteState)?;

        tx.commit()?;

        #[cfg(feature = "tracing")]
        tracing::debug!(number = block_number, hash = %block_hash);

        Ok(conn)
    };

    tokio::task::spawn_blocking(move || f).await?
}

/// Exit on critical errors, log recoverable errors
fn handle_error(e: InternalError) -> Result<(), CriticalError> {
    let e = map_recoverable_errors(e);
    match e {
        InternalError::Critical(e) => {
            #[cfg(feature = "tracing")]
            tracing::error!(
                "The state derivation stream has encountered a critical error: {} and cannot recover.",
                e
            );
            Err(e)
        }
        #[cfg(feature = "tracing")]
        InternalError::Recoverable(e) => {
            tracing::error!("The state derivation stream has encountered a recoverable error: {} and will now restart the stream.", e);

            Ok(())
        }
        #[cfg(not(feature = "tracing"))]
        InternalError::Recoverable(_) => Ok(()),
    }
}

/// Some critical error types contain variants that we should handle as recoverable errors.
/// This function maps those errors to recoverable errors.
fn map_recoverable_errors(e: InternalError) -> InternalError {
    // Map recoverable rusqlite errors to recoverable errors
    match e {
        InternalError::Critical(CriticalError::GetConnection(e)) => match e {
            rus @ rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error {
                    code: rusqlite::ffi::ErrorCode::DatabaseLocked,
                    ..
                },
                _,
            )
            | rus @ rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error {
                    code: rusqlite::ffi::ErrorCode::DatabaseBusy,
                    ..
                },
                _,
            ) => InternalError::Recoverable(error::RecoverableError::Rusqlite(rus)),
            _ => InternalError::Critical(CriticalError::GetConnection(e)),
        },
        _ => e,
    }
}
