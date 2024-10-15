use crate::{
    db::{self, ConnectionPool},
    error::{StateReadError, ValidationError},
};
use essential_check::{
    solution::{check_predicates, CheckPredicateConfig, PredicatesError},
    state_read_vm::{Gas, StateRead},
};
use essential_node_db::{
    finalized::{query_state_exclusive_solution, query_state_inclusive_solution},
    get_predicate,
};
use essential_types::{
    predicate::Predicate, solution::Solution, Block, ContentAddress, Key, PredicateAddress, Word,
};
use futures::FutureExt;
use std::{
    collections::{HashMap, HashSet},
    pin::Pin,
    sync::Arc,
};

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct State {
    block_number: Word,
    solution_index: u64,
    pre_state: bool,
    conn_pool: db::ConnectionPool,
}

/// Result of validating a block.
#[derive(Debug)]
pub enum ValidateOutcome {
    Valid(ValidOutcome),
    Invalid(InvalidOutcome),
}

/// Outcome of a valid block.
/// Cumulative gas and utilities of all solutions in the block.
#[derive(Debug)]
pub struct ValidOutcome {
    pub total_gas: Gas,
}

/// Outcome of an invalid block.
/// Contains the failure reason and the index of the solution that caused the failure.
#[derive(Debug)]
pub struct InvalidOutcome {
    pub failure: ValidateFailure,
    pub solution_index: usize,
}

/// Reasons for a block to be invalid.
/// Contains the error that caused the block to be invalid.
#[derive(Debug)]
pub enum ValidateFailure {
    #[allow(dead_code)]
    PredicatesError(PredicatesError<StateReadError>),
    GasOverflow,
}

/// Validates a solution.
/// Creates a block at the next block number and current timestamp with the given solution
/// and validates it.
///
/// Returns a `ValidationResult` if no `ValidationError` occurred that prevented the solution from being validated.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub async fn validate_solution(
    conn_pool: &ConnectionPool,
    solution: Solution,
) -> Result<ValidateOutcome, ValidationError> {
    let mut conn = conn_pool.acquire().await?;
    let tx = conn.transaction()?;
    let number = match essential_node_db::get_latest_finalized_block_address(&tx)? {
        Some(address) => essential_node_db::get_block_number(&tx, &address)?.unwrap_or(1),
        None => 1,
    };
    let block = Block {
        number,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time must be valid"),
        solutions: vec![solution],
    };
    drop(tx);
    validate(conn_pool, &block).await
}

/// Validates a block.
///
/// Returns a `ValidationResult` if no `ValidationError` occurred that prevented the block from being validated.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub async fn validate(
    conn_pool: &ConnectionPool,
    block: &Block,
) -> Result<ValidateOutcome, ValidationError> {
    // Read predicates from database.
    let predicate_addresses: HashSet<PredicateAddress> = block
        .solutions
        .iter()
        .flat_map(|solution| {
            solution
                .data
                .iter()
                .map(|data| data.predicate_to_solve.clone())
        })
        .collect();

    let mut conn = conn_pool.acquire().await?;

    let predicates = tokio::task::spawn_blocking(move || {
        let tx = conn.transaction()?;
        let mut predicates: HashMap<PredicateAddress, Arc<Predicate>> =
            HashMap::with_capacity(predicate_addresses.len());

        for predicate_address in predicate_addresses {
            let r = get_predicate(&tx, &predicate_address.predicate);

            match r {
                Ok(predicate) => match predicate {
                    Some(p) => {
                        predicates.insert(predicate_address, Arc::new(p));
                    }
                    None => {
                        return Err(ValidationError::PredicateNotFound(predicate_address));
                    }
                },
                Err(err) => {
                    return Err(ValidationError::Query(err));
                }
            }
        }
        Ok(predicates)
    })
    .await??;

    let mut total_gas: u64 = 0;

    // Check predicates.
    for (solution_index, solution) in block.solutions.iter().enumerate() {
        let pre_state = State {
            block_number: block.number,
            solution_index: solution_index as u64,
            pre_state: true,
            conn_pool: conn_pool.clone(),
        };
        let post_state = State {
            block_number: block.number,
            solution_index: solution_index as u64,
            pre_state: false,
            conn_pool: conn_pool.clone(),
        };
        let get_predicate = |addr: &PredicateAddress| {
            Arc::clone(
                predicates
                    .get(addr)
                    .expect("predicate must have been fetched in the previous step"),
            )
        };
        match check_predicates(
            &pre_state,
            &post_state,
            Arc::new(solution.clone()),
            get_predicate,
            Arc::new(CheckPredicateConfig::default()),
        )
        .await
        {
            Ok(g) => {
                if let Some(g) = total_gas.checked_add(g) {
                    total_gas = g;
                } else {
                    return Ok(ValidateOutcome::Invalid(InvalidOutcome {
                        failure: ValidateFailure::GasOverflow,
                        solution_index,
                    }));
                }
            }
            Err(err) => {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Validation failed for block with number {} and address {} at solution index {} with error {}", 
                    block.number,
                    essential_hash::content_addr(block),
                    solution_index,
                    err
                );
                return Ok(ValidateOutcome::Invalid(InvalidOutcome {
                    failure: ValidateFailure::PredicatesError(err),
                    solution_index,
                }));
            }
        }
    }

    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Validation successful for block with number {} and address {}. Gas: {}",
        block.number,
        essential_hash::content_addr(block),
        total_gas
    );
    Ok(ValidateOutcome::Valid(ValidOutcome { total_gas }))
}

impl StateRead for State {
    type Error = StateReadError;

    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<Vec<Vec<Word>>, Self::Error>> + Send>>;

    fn key_range(
        &self,
        contract_addr: ContentAddress,
        mut key: Key,
        num_values: usize,
    ) -> Self::Future {
        let Self {
            block_number,
            solution_index,
            pre_state,
            conn_pool,
        } = self.clone();

        async move {
            let mut conn = conn_pool.acquire().await?;

            tokio::task::spawn_blocking(move || {
                let tx = conn.transaction()?;
                let mut values = vec![];

                for _ in 0..num_values {
                    let value = if pre_state {
                        query_state_exclusive_solution(
                            &tx,
                            &contract_addr,
                            &key,
                            block_number,
                            solution_index,
                        )?
                        .unwrap_or_default()
                    } else {
                        query_state_inclusive_solution(
                            &tx,
                            &contract_addr,
                            &key,
                            block_number,
                            solution_index,
                        )?
                        .unwrap_or_default()
                    };
                    values.push(value);

                    key = next_key(key).ok_or_else(|| StateReadError::KeyRangeError)?;
                }
                Ok(values)
            })
            .await?
        }
        .boxed()
    }
}

/// Calculate the next key.
pub fn next_key(mut key: Key) -> Option<Key> {
    for w in key.iter_mut().rev() {
        match *w {
            Word::MAX => *w = Word::MIN,
            _ => {
                *w += 1;
                return Some(key);
            }
        }
    }
    None
}
