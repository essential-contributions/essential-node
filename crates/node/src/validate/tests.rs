use crate::{
    db::{finalize_block, insert_block, with_tx},
    test_utils::{
        register_contracts_block, test_big_bang, test_block_with_contracts, test_conn_pool,
        test_conn_pool_with_big_bang, test_invalid_block, test_invalid_block_with_contract,
    },
    validate::{self, InvalidOutcome, ValidOutcome, ValidateFailure, ValidateOutcome},
};
use essential_check::solution::{PredicateError, PredicatesError};
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

    let big_bang = test_big_bang();
    let contract_registry = big_bang.contract_registry.contract;
    let program_registry = big_bang.program_registry.contract;
    let outcome =
        validate::validate_dry_run(&conn_pool, &contract_registry, &program_registry, &block)
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

    let big_bang = test_big_bang();
    let contract_registry = big_bang.contract_registry.contract;
    let program_registry = big_bang.program_registry.contract;
    let outcome =
        validate::validate_dry_run(&conn_pool, &contract_registry, &program_registry, &block)
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
                            PredicateError::ConstraintsUnsatisfied(indices) => {
                                assert_eq!(indices.0.len(), 1);
                                assert_eq!(indices.0[0], 0);
                            }
                            _ => panic!(
                                "expected PredicateError::ConstraintsUnsatisfied, found {:?}",
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
    let (block, _, _) = test_invalid_block(0, Duration::from_secs(0));
    let big_bang = test_big_bang();
    let contract_registry = big_bang.contract_registry.contract;
    let program_registry = big_bang.program_registry.contract;
    let res =
        validate::validate_dry_run(&conn_pool, &contract_registry, &program_registry, &block).await;
    match res {
        Ok(ValidateOutcome::Invalid(InvalidOutcome {
            failure: ValidateFailure::MissingPredicate(addr),
            solution_index: 0,
        })) => {
            assert_eq!(addr, block.solutions[0].data[0].predicate_to_solve)
        }
        _ => panic!(
            "expected ValidateFailure::MissingPredicate, found {:?}",
            res
        ),
    }
}

#[tokio::test]
async fn program_not_found() {
    let conn_pool = test_conn_pool_with_big_bang().await;
    let mut conn = conn_pool.acquire().await.unwrap();

    let (block, contract, _) = test_invalid_block(1, Duration::from_secs(1));
    let big_bang = test_big_bang();
    let contract_registry = big_bang.contract_registry;
    let program_registry = big_bang.program_registry;

    // Register predicate.
    let register_block = register_contracts_block(
        contract_registry.clone(),
        Some(&contract),
        1,
        Duration::from_secs(1),
    )
    .unwrap();
    with_tx(&mut conn, |tx| {
        let block_ca = insert_block(tx, &register_block).unwrap();
        finalize_block(tx, &block_ca)
    })
    .unwrap();

    let res = validate::validate_dry_run(
        &conn_pool,
        &contract_registry.contract,
        &program_registry.contract,
        &block,
    )
    .await;
    match res {
        Ok(ValidateOutcome::Invalid(InvalidOutcome {
            failure: ValidateFailure::MissingProgram(addr),
            solution_index: 0,
        })) => {
            assert_eq!(addr, contract.predicates[0].nodes[0].program_address)
        }
        _ => panic!("expected ValidateFailure::MissingProgram, found {:?}", res),
    }
}

#[tokio::test]
async fn validate_dry_run() {
    let conn_pool = test_conn_pool_with_big_bang().await;

    // Insert a valid block with contracts.
    let block = test_block_with_contracts(1, Duration::from_secs(1));

    let big_bang = test_big_bang();
    let contract_registry = big_bang.contract_registry.contract;
    let program_registry = big_bang.program_registry.contract;
    let outcome =
        validate::validate_dry_run(&conn_pool, &contract_registry, &program_registry, &block)
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
