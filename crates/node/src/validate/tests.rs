use crate::{
    test_utils::{test_block, test_conn_pool, test_invalid_block},
    validate::{self, ValidationError},
};
use essential_hash::content_addr;
use essential_node_db::{create_tables, insert_contract};
use essential_types::contract::Contract;
use rusqlite::Connection;
use std::time::Duration;

fn insert_contracts_to_db(conn: &mut Connection, contracts: Vec<Contract>) {
    let tx = conn.transaction().unwrap();
    for contract in contracts {
        insert_contract(&tx, &contract, 0).unwrap();
    }
    tx.commit().unwrap();
}

#[tokio::test]
async fn valid_block() {
    let conn_pool = test_conn_pool("valid_block");
    let mut conn = conn_pool.acquire().await.unwrap();

    let tx = conn.transaction().unwrap();
    create_tables(&tx).unwrap();
    tx.commit().unwrap();

    let (block, contracts) = test_block(0, Duration::from_secs(0));
    insert_contracts_to_db(&mut conn, contracts);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let (validation_result, _) = validate::validate(&conn_pool, &block).await.unwrap();
    assert!(validation_result);
}

#[tokio::test]
async fn invalid_block() {
    let conn_pool = test_conn_pool("invalid_block");
    let mut conn = conn_pool.acquire().await.unwrap();

    let tx = conn.transaction().unwrap();
    create_tables(&tx).unwrap();
    tx.commit().unwrap();

    let (block, contract) = test_invalid_block(0, Duration::from_secs(0));
    insert_contracts_to_db(&mut conn, vec![contract]);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let (validation_result, failed_solution_hash) =
        validate::validate(&conn_pool, &block).await.unwrap();
    assert!(!validation_result);
    assert_eq!(
        failed_solution_hash,
        Some(content_addr(&block.solutions[0]))
    )
}

#[tokio::test]
async fn predicate_not_found() {
    let conn_pool = test_conn_pool("predicate_not_found");
    let mut conn = conn_pool.acquire().await.unwrap();

    let tx = conn.transaction().unwrap();
    create_tables(&tx).unwrap();
    tx.commit().unwrap();

    let (block, _) = test_invalid_block(0, Duration::from_secs(0));
    let res = validate::validate(&conn_pool, &block).await;

    match res {
        Err(ValidationError::PredicateNotFound(addr)) => {
            assert_eq!(addr, block.solutions[0].data[0].predicate_to_solve)
        }
        _ => panic!("expected predicate not found, found {:?}", res),
    }
}
