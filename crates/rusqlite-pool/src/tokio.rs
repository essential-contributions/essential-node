//! [`AsyncConnectionPool`] and [`AsyncConnectionHandle`] implemented using tokio semaphores.

use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::{AcquireError, OwnedSemaphorePermit, Semaphore, TryAcquireError};

/// A thin wrapper around a sync [`ConnectionPool`][crate::ConnectionPool]
/// providing orderly async access to DB connections via a [`Semaphore`].
pub struct AsyncConnectionPool {
    pool: crate::ConnectionPool,
    semaphore: Arc<Semaphore>,
}

/// A thin wrapper around a [`ConnectionHandle`][crate::ConnectionHandle] that
/// manages the associated permit that was provided by the pool's semaphore.
///
/// Upon `drop`, the connection handle is dropped first returning the connection
/// to the pool's queue, then the permit is dropped indicating avaialability to
/// the pool's semaphore.
pub struct AsyncConnectionHandle {
    // NOTE(MAINTAINERS):
    // These fields are intentionally ordered in drop order!
    // The connection must be dropped prior to the permit!
    conn: crate::ConnectionHandle,
    /// Hold the permit, so that it may be dropped immediately after the
    /// `ConnectionHandle` is dropped.
    _permit: OwnedSemaphorePermit,
}

/// One or more of the connections failed to close.
#[derive(Debug)]
pub struct AsyncCloseError(pub Vec<(Connection, rusqlite::Error)>);

impl AsyncConnectionPool {
    /// Create a new connection pool.
    ///
    /// All connections are created using `new_conn_fn` and queued for use prior
    /// to returning.
    ///
    /// If any of the connections fail to open, all previously successful
    /// connections (if any) are dropped and the error is returned.
    pub fn new<F>(conn_limit: usize, new_conn_fn: F) -> Result<Self, rusqlite::Error>
    where
        F: Fn() -> Result<Connection, rusqlite::Error>,
    {
        let pool = crate::ConnectionPool::new(conn_limit, new_conn_fn)?;
        let semaphore = Arc::new(Semaphore::new(conn_limit));
        Ok(Self { pool, semaphore })
    }

    /// The total number of simultaneous connections managed by the pool,
    /// specified by the user upon construction.
    pub fn capacity(&self) -> usize {
        self.pool.capacity()
    }

    /// Returns `true` if the inner idle queue is full, i.e. all `Connection`s
    /// are available for use.
    pub fn all_connections_ready(&self) -> bool {
        self.pool.all_connections_ready()
    }

    /// Acquire a connection from the idle queue.
    ///
    /// Awaits a permit from the inner semaphore before acquiring the connection.
    pub async fn acquire(&self) -> Result<AsyncConnectionHandle, AcquireError> {
        let _permit = Semaphore::acquire_owned(self.semaphore.clone()).await?;
        let conn = self.pool.pop().expect("permit guarantees availability");
        Ok(AsyncConnectionHandle { conn, _permit })
    }

    /// Attempt to acquire a connection from the idle queue if one is available.
    ///
    /// Returns a `TryAcquireError` if there are no permits available, or if the
    /// semaphore has been closed.
    pub fn try_acquire(&self) -> Result<AsyncConnectionHandle, TryAcquireError> {
        let _permit = Semaphore::try_acquire_owned(self.semaphore.clone())?;
        let conn = self.pool.pop().expect("permit guarantees availability");
        Ok(AsyncConnectionHandle { conn, _permit })
    }

    /// Acquire all connections, close the pool's semaphore and close all connections.
    ///
    /// All future calls to `acquire` or `try_acquire` will return immediately
    /// with `Err` following calling `close`.
    ///
    /// Returns `Ok(true)` in the case the pool was successfully closed.
    ///
    /// Returns `Ok(false)` in the case that the pool was already closed.
    ///
    /// Returns `Err` in the case that one or more connections failed to close.
    pub async fn close(&self) -> Result<bool, AsyncCloseError> {
        // Acquire all permits, awaiting as necessary.
        let capacity = u32::try_from(self.capacity()).expect("capacity out of range");
        let permit = match self.semaphore.acquire_many(capacity).await {
            // Can only be `Err` in the case that the pool was already closed.
            Err(_) => return Ok(false),
            Ok(permit) => permit,
        };

        // Disallow creating any further permits.
        self.semaphore.close();
        std::mem::drop(permit);

        // Close the connections and collect any errors that occurred.
        let results = self.pool.close();
        let errs: Vec<_> = results.into_iter().filter_map(Result::err).collect();
        if errs.is_empty() {
            Ok(true)
        } else {
            Err(AsyncCloseError(errs))
        }
    }

    /// Returns whether or not the pool has been closed.
    pub fn is_closed(&self) -> bool {
        self.semaphore.is_closed()
    }
}

impl core::ops::Deref for AsyncConnectionHandle {
    type Target = crate::ConnectionHandle;
    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

impl core::ops::DerefMut for AsyncConnectionHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.conn
    }
}

impl core::borrow::Borrow<Connection> for AsyncConnectionHandle {
    fn borrow(&self) -> &Connection {
        self
    }
}

impl core::borrow::BorrowMut<Connection> for AsyncConnectionHandle {
    fn borrow_mut(&mut self) -> &mut Connection {
        &mut *self
    }
}

impl core::fmt::Display for AsyncCloseError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        writeln!(f, "failed to close one or more connections:")?;
        for (ix, (_conn, err)) in self.0.iter().enumerate() {
            write!(f, "  {ix}: {err}")?;
        }
        Ok(())
    }
}

impl std::error::Error for AsyncCloseError {}
