use crate::{
    db::with_tx,
    error::{SolutionPredicatesError, ValidationError},
    test_utils::{
        test_block_with_contracts, test_conn_pool, test_conn_pool_with_big_bang,
        test_contract_registry, test_invalid_block, test_invalid_block_with_contract,
    },
    validate::{self, InvalidOutcome, ValidOutcome, ValidateFailure, ValidateOutcome},
};
use essential_check::{
    constraint_vm::error::CheckError,
    solution::{PredicateConstraintsError, PredicateError, PredicatesError},
};
use essential_node_db::{finalize_block, insert_block};
use std::time::Duration;

#[tokio::test]
async fn valid_block() {
    let conn_pool = test_conn_pool_with_big_bang().await;
    let mut conn = conn_pool.acquire().await.unwrap();

    // Insert a valid block with contracts.
    let block = test_block_with_contracts(1, Duration::from_secs(1));
    with_tx(&mut conn, |tx| {
        let block_ca = insert_block(tx, &block).unwrap();
        finalize_block(tx, &block_ca)
    })
    .unwrap();

    let contract_registry = test_contract_registry().contract;
    let outcome = validate::validate(&conn_pool, &contract_registry, &block)
        .await
        .unwrap();

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
    #[cfg(feature = "tracing")]
    let _ = tracing_subscriber::fmt::try_init();

    let conn_pool = test_conn_pool_with_big_bang().await;
    let mut conn = conn_pool.acquire().await.unwrap();

    // Insert an invalid block.
    let block = test_invalid_block_with_contract(1, Duration::from_secs(1));
    with_tx(&mut conn, |tx| {
        let block_ca = insert_block(tx, &block).unwrap();
        finalize_block(tx, &block_ca)
    })
    .unwrap();

    let contract_registry = test_contract_registry().contract;
    let outcome = validate::validate(&conn_pool, &contract_registry, &block)
        .await
        .unwrap();

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
    let contract_registry = test_contract_registry().contract;
    let res = validate::validate(&conn_pool, &contract_registry, &block).await;
    match res {
        Err(ValidationError::SolutionPredicates(SolutionPredicatesError::MissingPredicate(
            addr,
        ))) => {
            assert_eq!(addr, block.solutions[0].data[0].predicate_to_solve)
        }

        _ => panic!(
            "expected ValidationError::PredicateNotFound, found {:?}",
            res
        ),
    }
}
