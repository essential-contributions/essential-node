use essential_node::{self as node, Node};
use essential_node_api as node_api;
use std::future::Future;

const LOCALHOST: &str = "127.0.0.1";

#[cfg(feature = "tracing")]
pub fn init_tracing_subscriber() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .try_init();
}

pub fn test_node() -> Node {
    let mut conf = node::Config::default();
    conf.db.source = node::db::Source::Memory(uuid::Uuid::new_v4().into());
    Node::new(&conf).unwrap()
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .http2_prior_knowledge() // Enforce HTTP/2
        .build()
        .unwrap()
}

async fn test_listener() -> tokio::net::TcpListener {
    tokio::net::TcpListener::bind(format!("{LOCALHOST}:0"))
        .await
        .unwrap()
}

/// A function that waits until the server at the given port is ready to receive requests.
async fn await_server_online(port: u16, timeout_duration: std::time::Duration) {
    let server_ready = async {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
        let client = client();
        let url = format!("http://{LOCALHOST}:{port}/");
        loop {
            interval.tick().await;
            match client.get(&url).send().await {
                Ok(_) => return,
                Err(_) => continue, // Retry if the server is not ready yet
            }
        }
    };
    tokio::time::timeout(timeout_duration, server_ready)
        .await
        .unwrap()
}

/// Spawns a test server, then calls the given asynchronous function. Upon
/// completion, closes the server, panicking if any errors occurred.
pub async fn with_test_server<Fut>(
    state: node_api::State,
    f: impl FnOnce(u16) -> Fut,
) -> Fut::Output
where
    Fut: Future,
{
    let router = node_api::router(state);
    let listener = test_listener().await;
    let port = listener.local_addr().unwrap().port();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let api_jh = tokio::spawn(async move {
        tokio::select! {
            _ = node_api::serve(&router, &listener, node_api::DEFAULT_CONNECTION_LIMIT) => {},
            _ = shutdown_rx => {},
        }
    });
    await_server_online(port, std::time::Duration::from_secs(3)).await;
    let output = f(port).await;
    shutdown_tx.send(()).unwrap();
    api_jh.await.unwrap();
    output
}

pub fn get_url(port: u16, endpoint_path: &str) -> String {
    format!("http://{LOCALHOST}:{port}{endpoint_path}")
}

/// Shorthand for making a get request to the server instance at the given port for the given endpoint path.
pub async fn reqwest_get(port: u16, endpoint_path: &str) -> reqwest::Response {
    client()
        .get(get_url(port, endpoint_path))
        .send()
        .await
        .unwrap()
}

/// State that only has a DB connection pool and no new block TX (for non-subscription tests).
pub fn state_db_only(conn_pool: node::db::ConnectionPool) -> node_api::State {
    node_api::State {
        conn_pool,
        new_block: None,
    }
}
