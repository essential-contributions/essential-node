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
pub struct AsyncConnectionHandle {
    conn: crate::ConnectionHandle,
    permit: OwnedSemaphorePermit,
}

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

    /// Acquire a connection from the idle queue.
    ///
    /// Awaits a permit from the inner semaphore before acquiring the connection.
    pub async fn acquire(&self) -> Result<AsyncConnectionHandle, AcquireError> {
        let permit = Semaphore::acquire_owned(self.semaphore.clone()).await?;
        let conn = self.pool.pop().expect("permit guarantees availability");
        Ok(AsyncConnectionHandle { conn, permit })
    }

    /// Attempt to acquire a connection from the idle queue if one is available.
    ///
    /// Returns a `TryAcquireError` if there are no permits available, or if the
    /// semaphore has been closed.
    pub fn try_acquire(&self) -> Result<AsyncConnectionHandle, TryAcquireError> {
        let permit = Semaphore::try_acquire_owned(self.semaphore.clone())?;
        let conn = self.pool.pop().expect("permit guarantees availability");
        Ok(AsyncConnectionHandle { conn, permit })
    }

    /// Close the connection pool semaphore, await all connections to be
    /// returned and close all connections.
    ///
    /// Returns the [`Connection::close`][rusqlite::Connection::close] result
    /// for each connection in the queue.
    pub async fn close(self) -> Vec<Result<(), (Connection, rusqlite::Error)>> {
        // Acquire all of the permits.
        let capacity = u32::try_from(self.capacity()).expect("capacity out of range");
        let permit = self
            .semaphore
            .acquire_many(capacity)
            .await
            .expect("semaphore cannot be closed yet");

        // Disallow creating any further permits.
        self.semaphore.close();
        std::mem::drop(permit);

        // Close the connections.
        self.pool.close()
    }
}

impl AsyncConnectionHandle {
    /// Consume the `AsyncConnectionHandle` and provide access to the inner
    /// [`rusqlite::Connection`] via the given function.
    ///
    /// After the given function is called, the connection is immediately placed
    /// back on the pool's idle queue and the semaphore permit is released
    /// before returning.
    pub fn access<O>(self, f: impl FnOnce(&mut Connection) -> O) -> O {
        let output = self.conn.access(f);
        std::mem::drop(self.permit);
        output
    }
}
