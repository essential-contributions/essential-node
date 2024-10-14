use crate::{
    test_utils::{test_block, test_conn_pool, test_invalid_block},
    validate::{
        self, InvalidOutcome, ValidOutcome, ValidateFailure, ValidateOutcome, ValidationError,
    },
};
use essential_check::{
    constraint_vm::error::CheckError,
    solution::{PredicateConstraintsError, PredicateError, PredicatesError},
};
use essential_node_db::insert_contract;
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
    let conn_pool = test_conn_pool();
    let mut conn = conn_pool.acquire().await.unwrap();

    let (block, contracts) = test_block(0, Duration::from_secs(0));
    insert_contracts_to_db(&mut conn, contracts);

    let outcome = validate::validate(&conn_pool, &block).await.unwrap();

    match outcome {
        ValidateOutcome::Valid(ValidOutcome { total_gas }) => {
            assert!(total_gas > 0);
        }
        ValidateOutcome::Invalid(_) => {
            panic!("expected ValidateOutcome::Valid, found {:?}", outcome)
        }
    }
}

#[tokio::test]
async fn invalid_block() {
    let conn_pool = test_conn_pool();
    let mut conn = conn_pool.acquire().await.unwrap();

    let (block, contract) = test_invalid_block(0, Duration::from_secs(0));
    insert_contracts_to_db(&mut conn, vec![contract]);

    let outcome = validate::validate(&conn_pool, &block).await.unwrap();

    match outcome {
        ValidateOutcome::Invalid(InvalidOutcome {
            failure,
            solution_index,
        }) => {
            assert_eq!(solution_index, 0);
            match failure {
                ValidateFailure::PredicatesError(err) => match err {
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
                },
                _ => panic!(
                    "expected ValidateFailure::PredicatesError, found {:?}",
                    failure
                ),
            }
        }
        _ => {
            panic!("expected ValidateOutcome::Invalid, found {:?}", outcome)
        }
    }
}

#[tokio::test]
async fn predicate_not_found() {
    let conn_pool = test_conn_pool();

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
