//! A minimal connection pool for rusqlite.

use crossbeam::queue::ArrayQueue;
use rusqlite::Connection;
use std::sync::Arc;

#[cfg(feature = "tokio")]
pub mod tokio;

/// A pool of [`rusqlite::Connection`]s.
///
/// Internally, the pool is represented with a fixed-capacity, thread-safe
/// queue.
#[derive(Clone)]
pub struct ConnectionPool {
    queue: Arc<ArrayQueue<Connection>>,
}

/// A temporary handle to a [`rusqlite::Connection`] provided by a
/// [`ConnectionPool`].
///
/// Upon `drop`, the inner `Connection` is placed back in the pool's inner idle
/// queue for future use.
///
/// As a result, in async or multi-threaded environments, care should be taken
/// to avoid holding onto a `ConnectionHandle` any longer than necessary to
/// avoid blocking access to connections elsewhere.
pub struct ConnectionHandle {
    conn: Option<Connection>,
    queue: Arc<ArrayQueue<Connection>>,
}

const EXPECT_QUEUE_LEN: &str = "cannot exceed fixed queue size";
const EXPECT_CONN_SOME: &str = "connection cannot be `None`";

impl ConnectionPool {
    /// Create a new connection pool.
    ///
    /// This opens `capacity` number of connections using `new_conn_fn` and adds
    /// them to the inner queue.
    ///
    /// If any of the connections fail to open, all previously successful
    /// connections (if any) are dropped and the error is returned.
    pub fn new<F>(capacity: usize, new_conn_fn: F) -> Result<Self, rusqlite::Error>
    where
        F: Fn() -> Result<Connection, rusqlite::Error>,
    {
        let queue = Arc::new(ArrayQueue::new(capacity));
        for _ in 0..capacity {
            let conn = new_conn_fn()?;
            queue.push(conn).expect(EXPECT_QUEUE_LEN);
        }
        Ok(Self { queue })
    }

    /// Pop a connection from the queue if one is available.
    ///
    /// If `None` is returned, all connections are currently in use.
    ///
    /// The inner connection is returned to the pool upon dropping the handle.
    pub fn pop(&self) -> Option<ConnectionHandle> {
        self.queue.pop().map(|conn| ConnectionHandle {
            conn: Some(conn),
            queue: self.queue.clone(),
        })
    }

    /// The total number of simultaneous connections managed by the pool,
    /// specified by the user upon construction.
    pub fn capacity(&self) -> usize {
        self.queue.capacity()
    }

    /// Returns `true` if the inner idle queue is full, i.e. all `Connection`s
    /// are available for use.
    pub fn all_connections_ready(&self) -> bool {
        self.queue.is_full()
    }

    /// Manually close the pool and all connections in the inner queue.
    ///
    /// Returns the `Connection::close` result for each connection in the queue.
    ///
    /// If it is necessary that results are returned for all connections, care
    /// must be taken to ensure all [`ConnectionHandle`]s are dropped and that
    /// [`all_connections_ready`][Self::all_connections_ready] returns `true`
    /// before calling this method. Otherwise, connections not in the queue will
    /// be closed upon the last `ConnectionHandle` dropping.
    ///
    /// All connections closed during this call will be unavailable in future
    /// calls to [`pop`][ConnectionPool::pop].
    pub fn close(&self) -> Vec<Result<(), (Connection, rusqlite::Error)>> {
        let mut res = vec![];
        while let Some(conn) = self.queue.pop() {
            res.push(conn.close());
        }
        res
    }
}

impl AsRef<Connection> for ConnectionHandle {
    fn as_ref(&self) -> &Connection {
        self
    }
}

impl core::ops::Deref for ConnectionHandle {
    type Target = Connection;
    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().expect(EXPECT_CONN_SOME)
    }
}

impl core::ops::DerefMut for ConnectionHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn.as_mut().expect(EXPECT_CONN_SOME)
    }
}

impl core::borrow::Borrow<Connection> for ConnectionHandle {
    fn borrow(&self) -> &Connection {
        self
    }
}

impl core::borrow::BorrowMut<Connection> for ConnectionHandle {
    fn borrow_mut(&mut self) -> &mut Connection {
        &mut *self
    }
}

impl Drop for ConnectionHandle {
    fn drop(&mut self) {
        // Return the connection to the pool's queue.
        let conn = self.conn.take().expect(EXPECT_CONN_SOME);
        self.queue.push(conn).expect(EXPECT_QUEUE_LEN);
    }
}
