#![cfg(feature = "test-utils")]

use essential_node::{
    test_utils::{setup_server, test_blocks, test_db_conf},
    Node,
};
use essential_types::contract::{Contract, SignedContract};

#[tokio::test]
async fn test_run() {
    std::env::set_var(
        "RUST_LOG",
        "[deploy]=trace,[submit_solution]=trace,[run_loop]=trace,[run_blocks]=trace",
    );
    #[cfg(feature = "tracing")]
    let _ = tracing_subscriber::fmt::try_init();

    // Setup node
    let conf = test_db_conf("test_acquire");
    let node = Node::new(&conf).unwrap();

    // Setup server
    let (server_address, _child) = setup_server().await;
    let client = reqwest::ClientBuilder::new()
        .http2_prior_knowledge()
        .build()
        .unwrap();
    let url = reqwest::Url::parse(server_address.as_str())
        .unwrap()
        .join("/deploy-contract")
        .unwrap();
    let solve_url = reqwest::Url::parse(server_address.as_str())
        .unwrap()
        .join("/submit-solution")
        .unwrap();

    // Run server
    let db = node.db();
    let _handle = node.run(server_address).await.unwrap();

    // Create test blocks
    let (test_blocks, test_contracts) = test_blocks(2);

    // Deploy contracts to server
    for contract in &test_contracts {
        let r = client
            .post(url.clone())
            .json(&sign(contract.clone()))
            .send()
            .await
            .unwrap();
        assert!(r.status().is_success(), "{}", r.text().await.unwrap());
    }
    // Deploy first solution to server
    let r = client
        .post(solve_url.clone())
        .json(&test_blocks[0].solutions[0])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{}", r.text().await.unwrap());
    tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;

    // Check if the block was added to the database
    let conn = db.acquire().await.unwrap();
    let blocks = &essential_node_db::list_blocks(&conn, 0..1).unwrap();
    assert_eq!(blocks[0].solutions[0], test_blocks[0].solutions[0].clone());
}

fn sign(contract: Contract) -> SignedContract {
    let secp = secp256k1::Secp256k1::new();
    let key = secp.generate_keypair(&mut secp256k1::rand::rngs::OsRng).0;
    essential_sign::contract::sign(contract, &key)
}
