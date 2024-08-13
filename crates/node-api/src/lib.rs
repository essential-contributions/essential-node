use axum::{routing::get, Router};
use essential_node::db;
use std::{io, net::SocketAddr};
use thiserror::Error;
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinSet,
};
use tower_http::cors::CorsLayer;

pub mod endpoint;

/// An error occurred while attempting to serve a new connection.
#[derive(Debug, Error)]
pub enum ServeNextConnError {
    /// Failed to acquire the next connection.
    #[error("failed to acquire next connection: {0}")]
    Next(#[from] io::Error),
    /// Failed to serve the connection.
    #[error("{0}")]
    Serve(#[from] ServeConnError),
}

/// An error occurred while attempting to serve a connection.
#[derive(Debug, Error)]
#[error("Serve connection error: {0}")]
pub struct ServeConnError(#[from] Box<dyn std::error::Error + Send + Sync>);

/// The maximum number of connections to maintain at once.
pub const CONNECTION_LIMIT: usize = 2_000;

/// Accept and serve the next connection.
///
/// If we're at the connection limit, this first awaits for a connection task to
/// become available.
///
/// ```no_run
/// # #[tokio::main]
/// # async fn main() {
/// # use essential_node::{self as node, Node};
/// # use essential_node_api as node_api;
/// let conf = node::Config::default();
/// let node = Node::new(&conf).unwrap();
/// let router = node_api::router(node.db());
/// let listener = tokio::net::TcpListener::bind("127.0.0.1:3553").await.unwrap();
/// let mut conn_set = tokio::task::JoinSet::new();
/// // Accept and serve connections.
/// loop {
///     node_api::serve_next_conn(&router, &listener, &mut conn_set).await;
/// }
/// # }
pub async fn serve_next_conn(router: &Router, listener: &TcpListener, conn_set: &mut JoinSet<()>) {
    // Await the next connection.
    let stream = match next_conn(&listener, conn_set).await {
        Ok((stream, _remote_addr)) => {
            #[cfg(feature = "tracing")]
            tracing::trace!("Accepted new connection from: {_remote_addr}");
            stream
        }
        Err(_err) => {
            #[cfg(feature = "tracing")]
            tracing::trace!("Failed to accept connection {_err}");
            return;
        }
    };

    // Serve the acquired connection.
    if let Err(_err) = serve_conn(&router.clone(), stream).await {
        #[cfg(feature = "tracing")]
        tracing::trace!("Serve connection error: {_err}");
    }
}

/// Accept and return the next TCP stream connection.
///
/// If we're at the connection limit, this first awaits for a connection task to
/// become available.
pub async fn next_conn(
    listener: &TcpListener,
    conn_set: &mut JoinSet<()>,
) -> io::Result<(TcpStream, SocketAddr)> {
    // If the `conn_set` size currently exceeds the limit, wait for the next to join.
    if conn_set.len() >= CONNECTION_LIMIT {
        #[cfg(feature = "tracing")]
        tracing::info!("Connection limit reached: {CONNECTION_LIMIT}");
        conn_set.join_next().await.expect("set cannot be empty")?;
    }
    // Await another connection.
    listener.accept().await
}

/// Serve a newly accepted TCP stream.
pub async fn serve_conn(router: &Router, stream: TcpStream) -> Result<(), ServeConnError> {
    // Hyper has its own `AsyncRead` and `AsyncWrite` traits and doesn't use
    // tokio. `TokioIo` converts between them.
    let stream = hyper_util::rt::TokioIo::new(stream);

    // Hyper also has its own `Service` trait and doesn't use tower. We can use
    // `hyper::service::service_fn` to create a hyper `Service` that calls our
    // app through `tower::Service::call`.
    let hyper_service = hyper::service::service_fn(
        move |request: axum::extract::Request<hyper::body::Incoming>| {
            tower::Service::call(&mut router.clone(), request)
        },
    );

    // `TokioExecutor` tells hyper to use `tokio::spawn` to spawn tasks.
    let executor = hyper_util::rt::TokioExecutor::new();
    let conn = hyper_util::server::conn::auto::Builder::new(executor).http2_only();
    conn.serve_connection(stream, hyper_service)
        .await
        .map_err(ServeConnError)
}

/// Construct the endpoint router.
pub fn router(conn_pool: db::ConnectionPool) -> Router {
    use endpoint::*;
    Router::new()
        .route(health_check::PATH, get(health_check::handler))
        .route(get_contract::PATH, get(get_contract::handler))
        .route(get_predicate::PATH, get(get_predicate::handler))
        .route(list_blocks::PATH, get(list_blocks::handler))
        .route(list_contracts::PATH, get(list_contracts::handler))
        .route(query_state::PATH, get(query_state::handler))
        .layer(cors_layer())
        .with_state(conn_pool)
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([http::Method::GET, http::Method::OPTIONS])
        .allow_headers([http::header::CONTENT_TYPE])
}
