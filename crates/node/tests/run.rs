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
    solution::Solution,
    Block, ContentAddress,
};
use reqwest::{Client, Url};
use rusqlite::Connection;

// Submit provided solutions to server one by one.
async fn submit_solutions(client: &Client, solve_url: &Url, solutions: &Vec<Solution>) {
    for solution in solutions {
        let r = client
            .post(solve_url.clone())
            .json(solution)
            .send()
            .await
            .unwrap();
        assert!(r.status().is_success(), "{}", r.text().await.unwrap());
    }
}

// Fetch blocks from node database and assert that they contain the same solutions as expected.
// Assert state mutations in the blocks have been applied to database.
// Assert state progress is the latest fetched block.
fn assert_submit_solutions_effects(conn: &Connection, expected_blocks: Vec<Block>) {
    let fetched_blocks = &essential_node_db::list_blocks(
        &conn,
        expected_blocks[0].number..expected_blocks[expected_blocks.len() - 1].number + 1,
    )
    .unwrap();
    for (i, expected_block) in expected_blocks.iter().enumerate() {
        // Check if the block was added to the database
        assert_eq!(fetched_blocks[i].number, expected_block.number);
        assert_eq!(
            fetched_blocks[i].solutions.len(),
            expected_block.solutions.len()
        );
        for (j, fetched_block_solution) in fetched_blocks[i].solutions.iter().enumerate() {
            assert_eq!(fetched_block_solution, &expected_block.solutions[j].clone())
        }
        // Assert mutations in block are in database
        assert_multiple_block_mutations(&conn, &[&fetched_blocks[i]]);
    }
    // Assert state progress is latest block
    assert_state_progress_is_some(
        &conn,
        &fetched_blocks[fetched_blocks.len() - 1],
        &essential_hash::content_addr(&fetched_blocks[fetched_blocks.len() - 1]),
    );
}

#[tokio::test]
async fn test_run() {
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

    // Run node
    let db = node.db();
    let _handle = node.run(server_address).await.unwrap();

    // Create test blocks
    let test_blocks_count = 4;
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
    submit_solutions(&client, &solve_url, &test_blocks[0].solutions).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;

    // Check block, state and state progress
    let conn = db.acquire().await.unwrap();
    assert_submit_solutions_effects(&conn, vec![test_blocks[0].clone()]);

    // Submit test block 1 and 2's solutions to server
    submit_solutions(&client, &solve_url, &test_blocks[1].solutions).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;
    submit_solutions(&client, &solve_url, &test_blocks[2].solutions).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;

    // Check block, state and state progress
    let conn = db.acquire().await.unwrap();
    assert_submit_solutions_effects(&conn, vec![test_blocks[1].clone(), test_blocks[2].clone()]);

    // Submit test block 3's solutions to server
    submit_solutions(&client, &solve_url, &test_blocks[3].solutions).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;

    // Check block, state and state progress
    let conn = db.acquire().await.unwrap();
    assert_submit_solutions_effects(&conn, vec![test_blocks[3].clone()]);
}

fn sign(contract: Contract) -> SignedContract {
    let secp = secp256k1::Secp256k1::new();
    let key = secp.generate_keypair(&mut secp256k1::rand::rngs::OsRng).0;
    essential_sign::contract::sign(contract, &key)
}
