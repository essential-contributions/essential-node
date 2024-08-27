use essential_node as node;
use essential_node_api as node_api;
use essential_types::{
    contract::Contract, convert::bytes_from_word, predicate::Predicate, Block, Value, Word,
};
use util::{client, get_url, init_tracing_subscriber, reqwest_get, test_node, with_test_server};

mod util;

#[tokio::test]
async fn test_health_check() {
    #[cfg(feature = "tracing")]
    init_tracing_subscriber();

    let node = test_node("test_health_check");
    with_test_server(node.db(), |port| async move {
        let response = reqwest_get(port, node_api::endpoint::health_check::PATH).await;
        assert!(response.status().is_success());
    })
    .await;
}

#[tokio::test]
async fn test_get_contract() {
    #[cfg(feature = "tracing")]
    init_tracing_subscriber();

    let node = test_node("test_get_contract");

    // The test contract.
    let seed = 42;
    let da_block = 100;
    let contract = std::sync::Arc::new(node::test_utils::test_contract(seed));
    let contract_ca = essential_hash::content_addr(contract.as_ref());

    // Insert the contract.
    let clone = contract.clone();
    node.db().insert_contract(clone, da_block).await.unwrap();

    // Request the contract from the server.
    let response_contract = with_test_server(node.db(), |port| async move {
        let response = reqwest_get(port, &format!("/get-contract/{contract_ca}")).await;
        assert!(response.status().is_success());
        response.json::<Contract>().await.unwrap()
    })
    .await;

    assert_eq!(&*contract, &response_contract);
}

#[tokio::test]
async fn test_get_contract_invalid_ca() {
    #[cfg(feature = "tracing")]
    init_tracing_subscriber();

    let node = test_node("test_get_contract_invalid_ca");

    // The test contract.
    let seed = 78;
    let da_block = 100;
    let contract = std::sync::Arc::new(node::test_utils::test_contract(seed));

    // Insert the contract.
    let clone = contract.clone();
    node.db().insert_contract(clone, da_block).await.unwrap();

    // Request the contract from the server.
    with_test_server(node.db(), |port| async move {
        let response = reqwest_get(port, "/get-contract/INVALID_CA").await;
        assert!(response.status().is_client_error());
    })
    .await;
}

#[tokio::test]
async fn test_get_predicate() {
    #[cfg(feature = "tracing")]
    init_tracing_subscriber();

    let node = test_node("test_get_predicate");

    // The test contract.
    let seed = 97;
    let da_block = 100;
    let contract = std::sync::Arc::new(node::test_utils::test_contract(seed));

    // Insert the contract.
    let clone = contract.clone();
    node.db().insert_contract(clone, da_block).await.unwrap();

    // Check that we can request each predicate individually.
    with_test_server(node.db(), |port| async move {
        for predicate in &contract.predicates {
            let predicate_ca = essential_hash::content_addr(predicate);
            let response = reqwest_get(port, &format!("/get-predicate/{predicate_ca}")).await;
            assert!(response.status().is_success());
            let response_predicate = response.json::<Predicate>().await.unwrap();
            assert_eq!(predicate, &response_predicate);
        }
    })
    .await;
}

#[tokio::test]
async fn test_query_state() {
    #[cfg(feature = "tracing")]
    init_tracing_subscriber();

    let node = test_node("test_query_state");

    // The test state.
    let seed = 11;
    let da_block = 100;
    let contract = std::sync::Arc::new(node::test_utils::test_contract(seed));

    // Make some randomish keys and values.
    let mut keys = vec![];
    let mut values = vec![];
    for i in 0i64..256 {
        let key = vec![(i + 1) * 5; 1 + (i as usize * 103) % 128];
        let value = vec![(i + 1) * 7; 1 + (i as usize * 391) % 128];
        keys.push(key);
        values.push(value);
    }

    // Insert a contract to own the state.
    node.db()
        .insert_contract(contract.clone(), da_block)
        .await
        .unwrap();
    let contract_ca = essential_hash::content_addr(contract.as_ref());

    // Insert the state entries.
    for (k, v) in keys.iter().zip(&values) {
        let db = node.db();
        let ca = contract_ca.clone();
        let (key, value) = (k.clone(), v.clone());
        db.update_state(ca, key, value).await.unwrap();
    }

    // Query each of the keys and check they match what we expect.
    with_test_server(node.db(), |port| async move {
        for (k, v) in keys.iter().zip(&values) {
            let key_bytes: Vec<_> = k.iter().copied().flat_map(bytes_from_word).collect();
            let key = hex::encode(&key_bytes);
            let response = reqwest_get(port, &format!("/query-state/{contract_ca}/{key}")).await;
            assert!(response.status().is_success());
            let response_value = response.json::<Option<Value>>().await.unwrap();
            assert_eq!(Some(v), response_value.as_ref());
        }
    })
    .await;
}

#[tokio::test]
async fn test_list_blocks() {
    #[cfg(feature = "tracing")]
    init_tracing_subscriber();

    let node = test_node("test_list_blocks");

    // Create some test blocks.
    let n_blocks = 100;
    let (blocks, _) = node::test_utils::test_blocks(n_blocks);

    // Insert them into the node's DB.
    for block in &blocks {
        node.db()
            .insert_block(std::sync::Arc::new(block.clone()))
            .await
            .unwrap();
    }

    // Fetch all blocks.
    let fetched_blocks = with_test_server(node.db(), |port| async move {
        let response = client()
            .get(get_url(
                port,
                &format!("/list-blocks?start={}&end={}", 0, n_blocks),
            ))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        response.json::<Vec<Block>>().await.unwrap()
    })
    .await;

    assert_eq!(blocks, fetched_blocks);
}

#[tokio::test]
async fn test_list_contracts() {
    #[cfg(feature = "tracing")]
    init_tracing_subscriber();

    let node = test_node("test_list_contracts");

    // The contract seeds for each block.
    let block_contract_seeds: &[&[Word]] = &[&[1], &[42, 69], &[1337, 7357, 9000], &[4]];

    // The list of contracts per block.
    let block_contracts: Vec<Vec<Contract>> = block_contract_seeds
        .iter()
        .map(|seeds| {
            seeds
                .iter()
                .copied()
                .map(node::test_utils::test_contract)
                .collect::<Vec<_>>()
        })
        .collect();

    // Create the necessary tables and insert contracts.
    for (ix, contracts) in block_contracts.iter().enumerate() {
        let block_n = ix.try_into().unwrap();
        for contract in contracts {
            let contract = std::sync::Arc::new(contract.clone());
            node.db().insert_contract(contract, block_n).await.unwrap();
        }
    }

    // Query the second and third blocks.
    let start = 1;
    let end = 3;

    // Fetch the blocks.
    let fetched_contracts = with_test_server(node.db(), |port| async move {
        let response = client()
            .get(get_url(
                port,
                &format!("/list-contracts?start={}&end={}", start, end),
            ))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        response.json::<Vec<(u64, Vec<Contract>)>>().await.unwrap()
    })
    .await;

    // Check the fetched contracts match the expected range.
    let expected = &block_contracts[start as usize..end as usize];
    for ((ix, expected), (block, contracts)) in expected.iter().enumerate().zip(&fetched_contracts)
    {
        assert_eq!(ix as u64 + start, *block);
        assert_eq!(expected, contracts);
    }
}
