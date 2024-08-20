#![cfg(feature = "test-utils")]

use essential_node::{
    test_utils::{
        assert_multiple_block_mutations, assert_state_progress_is_none,
        assert_state_progress_is_some, setup_server, test_blocks, test_db_conf,
    },
    Node,
};
use essential_types::{
    contract::{Contract, SignedContract},
    ContentAddress,
};

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
    let test_blocks_count = 2;
    let (test_blocks, test_contracts) = test_blocks(test_blocks_count);

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
    tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;
    let conn = db.acquire().await.unwrap();
    let fetched_contracts: Vec<ContentAddress> = essential_node_db::list_contracts(&conn, 0..100)
        .unwrap()
        .into_iter()
        .map(|(_, contracts)| {
            contracts
                .into_iter()
                .map(|c| essential_hash::contract_addr::from_contract(&c))
        })
        .flatten()
        .collect();
    assert_eq!(fetched_contracts.len(), test_contracts.len());
    for test_contract in test_contracts.iter() {
        let hash = essential_hash::contract_addr::from_contract(&test_contract);
        assert!(fetched_contracts.contains(&hash));
    }

    // Initially, the state progress is none
    assert_state_progress_is_none(&conn);

    // Submit test block 0's solutions to server
    for solution in &test_blocks[0].solutions {
        let r = client
            .post(solve_url.clone())
            .json(solution)
            .send()
            .await
            .unwrap();
        assert!(r.status().is_success(), "{}", r.text().await.unwrap());
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;

    // Check if the block was added to the database
    let conn = db.acquire().await.unwrap();
    let fetched_blocks = &essential_node_db::list_blocks(&conn, 0..1).unwrap();
    assert_eq!(fetched_blocks[0].number, test_blocks[0].number);
    assert_eq!(
        fetched_blocks[0].solutions.len(),
        test_blocks[0].solutions.len()
    );
    for (i, fetched_block_solution) in fetched_blocks[0].solutions.iter().enumerate() {
        assert_eq!(fetched_block_solution, &test_blocks[0].solutions[i].clone())
    }
    // Assert state progress is block 0
    assert_state_progress_is_some(
        &conn,
        &fetched_blocks[0],
        &essential_hash::content_addr(&fetched_blocks[0]),
    );
    // Assert mutations in block 0 are in database
    assert_multiple_block_mutations(&conn, &[&fetched_blocks[0]]);
}

fn sign(contract: Contract) -> SignedContract {
    let secp = secp256k1::Secp256k1::new();
    let key = secp.generate_keypair(&mut secp256k1::rand::rngs::OsRng).0;
    essential_sign::contract::sign(contract, &key)
}
