use crate::{
    test_utils::{test_block, test_conn_pool, test_invalid_block},
    validate::{self, ValidationError},
};
use essential_check::{
    constraint_vm::error::CheckError,
    solution::{PredicateConstraintsError, PredicateError, PredicatesError},
};
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

    let (_utility, _gas) = validate::validate(&conn_pool, &block)
        .await
        .unwrap()
        .unwrap();
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

    let (err, index) = validate::validate(&conn_pool, &block)
        .await
        .unwrap()
        .err()
        .unwrap();

    assert_eq!(index, 0);
    match err {
        PredicatesError::Failed(errs) => {
            assert_eq!(errs.0.len(), 1);
            let (solution_data_index, predicate_err) = &errs.0[0];
            assert_eq!(*solution_data_index, 0);
            match predicate_err {
                PredicateError::Constraints(err) => match err {
                    PredicateConstraintsError::Check(err) => match err {
                        CheckError::ConstraintsUnsatisfied(indices) => {
                            assert_eq!(indices.0.len(), 1);
                            assert_eq!(indices.0[0], 0);
                        }
                        _ => panic!(
                            "expected CheckError::ConstraintsUnsatisfied, found {:?}",
                            predicate_err
                        ),
                    },
                    _ => panic!(
                        "expected PredicateConstraintsError::Check, found {:?}",
                        predicate_err
                    ),
                },
                _ => panic!(
                    "expected PredicateError::Constraints, found {:?}",
                    predicate_err
                ),
            }
        }
        _ => panic!("expected PredicatesError::Failed, found {:?}", err),
    }
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
        _ => panic!(
            "expected ValidationError::PredicateNotFound, found {:?}",
            res
        ),
    }
}
