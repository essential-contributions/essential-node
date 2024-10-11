# rusqlite-pool

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![license][apache-badge]][apache-url]
[![Build Status][actions-badge]][actions-url]

[crates-badge]: https://img.shields.io/crates/v/rusqlite-pool.svg
[crates-url]: https://crates.io/crates/rusqlite-pool
[docs-badge]: https://docs.rs/rusqlite-pool/badge.svg
[docs-url]: https://docs.rs/rusqlite-pool
[apache-badge]: https://img.shields.io/badge/license-APACHE-blue.svg
[apache-url]: LICENSE
[actions-badge]: https://github.com/essential-contributions/essential-node/workflows/ci/badge.svg
[actions-url]: https://github.com/essential-contributions/essential-node/actions

A minimal connection pool for rusqlite, suitable for async and multi-threaded usage.

Provides connection pools and handles for sync and async usages:

-   `ConnectionPool`: A fixed-capacity, thread-safe queue of [`rusqlite::Connection`](https://docs.rs/rusqlite/latest/rusqlite/struct.Connection.html#)s available for usage.
-   `ConnectionHandle`: A temporary handle to a connection.
-   `AsyncConnectionPool`: A thin wrapper around `ConnectionPool` that uses [Semaphore](https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html)s for orderly async access to database connections.
-   `AsyncConnectionHandle`: A thin wrapper around `ConnectionHandle` that manages the associated [SemaphorePermit](https://docs.rs/tokio/latest/tokio/sync/struct.SemaphorePermit.html).

The async counterparts are behind feature `tokio`.
