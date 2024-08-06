use crate::{
    error::{CriticalError, InternalError, RecoverableError},
    handle::Handle,
};
use essential_node_db::{get_state_progress, list_blocks, update_state, update_state_progress};
use essential_types::{solution::Mutation, Block, ContentAddress};
use futures::stream::{StreamExt, TryStreamExt};
use std::{borrow::BorrowMut, future::Future};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

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
    let next_block_number = progress
        .as_ref()
        .map(|(block_number, _)| block_number.saturating_add(1))
        .unwrap_or(0);

    let (conn, block) = get_next_block(conn, progress, next_block_number).await?;
    let block_hash = essential_hash::content_addr(&block);

    let mutations: Vec<Mutations<Vec<Mutation>>> = block
        .solutions
        .into_iter()
        .map(|solution| {
            solution.data.into_iter().map(|data| Mutations {
                contract_address: data.predicate_to_solve.contract,
                mutations: data.state_mutations,
            })
        })
        .flatten()
        .collect();

    Ok(update_state_in_db(conn, mutations, block.number, &block_hash).await?)
}

async fn get_last_progress<C>(
    conn: C,
) -> Result<(C, Option<(u64, ContentAddress)>), RecoverableError>
where
    C: BorrowMut<rusqlite::Connection> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let r = get_state_progress(conn.borrow())
            .map_err(|_err| RecoverableError::LastProgressError)?;
        Ok((conn, r))
    })
    .await?
}

async fn get_next_block<C>(
    conn: C,
    progress: Option<(u64, ContentAddress)>,
    next_block_number: u64,
) -> Result<(C, Block), InternalError>
where
    C: BorrowMut<rusqlite::Connection> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let range = progress.as_ref().map_or(
            next_block_number..next_block_number.saturating_add(1),
            |(block_num, _)| *block_num..next_block_number.saturating_add(1),
        );

        let blocks = list_blocks(conn.borrow(), range).map_err(RecoverableError::ReadStateError)?;

        let block = match progress {
            Some((_, hash)) => {
                let mut iter = blocks.into_iter();
                let previous_block = iter.next().ok_or(CriticalError::Fork)?;
                let current_block = iter.next().ok_or(RecoverableError::BlockNotFound)?;
                if essential_hash::content_addr(&previous_block) != hash {
                    return Err(CriticalError::Fork.into());
                }
                current_block
            }
            None => blocks
                .into_iter()
                .next()
                .ok_or(RecoverableError::BlockNotFound)?,
        };

        Ok((conn, block))
    })
    .await
    .map_err(RecoverableError::JoinError)?
}

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
    let block_hash = block_hash.clone();

    tokio::task::spawn_blocking(move || {
        let tx = conn.borrow_mut().transaction()?;

        for mutation in mutations {
            for m in mutation.mutations {
                update_state(&tx, &mutation.contract_address, &m.key, &m.value)
                    .map_err(|err| RecoverableError::WriteStateError(err))?;
            }

            update_state_progress(&tx, block_number, &block_hash)
                .map_err(|err| RecoverableError::WriteStateError(err))?;
        }
        tx.commit()?;

        Ok(conn)
    })
    .await?
}

/// Exit on critical errors, log recoverable errors
fn handle_error(e: InternalError) -> Result<(), CriticalError> {
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
