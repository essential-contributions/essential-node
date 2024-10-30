use std::time::Duration;

use super::*;

#[tokio::test]
async fn test_args() {
    let args = Args::parse_from(Some(""));
    let r = tokio::time::timeout(Duration::from_millis(100), run(args.clone())).await;
    // Error means the timeout happened thus run was successful.
    assert!(r.is_err(), "{:?}", r);

    // Bad relayer source endpoint.
    let mut a = args.clone();
    a.relayer_source_endpoint = Some("http://0.0.0.0:0".to_string());
    let r = tokio::time::timeout(Duration::from_millis(100), run(a.clone())).await;

    // Error means the timeout happened thus run was successful.
    // Relayer should still run and continue to try to connect to source while
    // logging errors.
    assert!(r.is_err(), "{:?}", r);

    // Good relayer source endpoint.
    let mut a = args.clone();
    let (_api, port) = test_node().await;
    a.relayer_source_endpoint = Some(format!("http://127.0.0.1:{}", port));

    let r = tokio::time::timeout(Duration::from_millis(100), run(a.clone())).await;
    // Error means the timeout happened thus run was successful.
    assert!(r.is_err(), "{:?}", r);

    // Disable all streams
    let mut a = args.clone();
    a.disable_validation = true;
    a.relayer_source_endpoint = None;

    let r = tokio::time::timeout(Duration::from_millis(100), run(a.clone())).await;
    // Error means the timeout happened thus run was successful.
    assert!(r.is_err(), "{:?}", r);
}

async fn test_node() -> (impl std::future::Future<Output = ()>, u16) {
    let block_tx = node::BlockTx::new();
    let block_rx = block_tx.new_listener();
    let config = node::db::Config {
        source: node::db::Source::Memory(uuid::Uuid::new_v4().to_string()),
        ..Default::default()
    };
    let db = node::db(&config).unwrap();
    let api_state = node_api::State {
        new_block: Some(block_rx),
        conn_pool: db.clone(),
    };
    let router = node_api::router(api_state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let api = async move {
        node_api::serve(&router, &listener, 2).await;
    };
    (api, port)
}
