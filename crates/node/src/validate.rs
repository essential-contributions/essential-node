//! # Validation
//! Functions for validating blocks and solutions.
use crate::{
    db::{self, ConnectionPool},
    error::{QueryPredicateError, SolutionPredicatesError, StateReadError, ValidationError},
};
use essential_check::{
    solution::{check_predicates, CheckPredicateConfig, PredicatesError},
    state_read_vm::{Gas, StateRead},
};
use essential_node_db::{
    finalized::{query_state_exclusive_solution, query_state_inclusive_solution},
    QueryError,
};
use essential_types::{
    convert::bytes_from_word, predicate::Predicate, solution::Solution, solution::SolutionData,
    Block, ContentAddress, Key, PredicateAddress, Value, Word,
};
use futures::FutureExt;
use std::{collections::HashMap, pin::Pin, sync::Arc};

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
    /// The block is valid.
    Valid(ValidOutcome),
    /// The block is invalid.
    Invalid(InvalidOutcome),
}

/// Outcome of a valid block.
/// Cumulative gas and utilities of all solutions in the block.
#[derive(Debug)]
pub struct ValidOutcome {
    /// Total gas consumed by all solutions in the block.
    pub total_gas: Gas,
}

/// Outcome of an invalid block.
/// Contains the failure reason and the index of the solution that caused the failure.
#[derive(Debug)]
pub struct InvalidOutcome {
    /// The reason for the block to be invalid.
    pub failure: ValidateFailure,
    /// The index of the solution that caused the failure.
    pub solution_index: usize,
}

/// Reasons for a block to be invalid.
/// Contains the error that caused the block to be invalid.
#[derive(Debug)]
pub enum ValidateFailure {
    /// A solution specified a predicate that does not exist within the contract registry.
    MissingPredicate(PredicateAddress),
    /// A predicate was present in the registry, but failed to decode.
    InvalidPredicate(PredicateAddress),
    #[allow(dead_code)]
    /// A predicate failed to validate.
    PredicatesError(PredicatesError<StateReadError>),
    /// The total gas consumed by all solutions in the block exceeds the maximum gas limit.
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
    contract_registry: &ContentAddress,
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
    validate(conn_pool, contract_registry, &block).await
}

/// Validates a block.
///
/// Returns a `ValidationResult` if no `ValidationError` occurred that prevented the block from being validated.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub async fn validate(
    conn_pool: &ConnectionPool,
    contract_registry: &ContentAddress,
    block: &Block,
) -> Result<ValidateOutcome, ValidationError> {
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

        // Create the `predicates` map.
        let res = query_solution_predicates(&post_state, contract_registry, &solution.data).await;
        let predicates = match res {
            Ok(predicates) => Arc::new(predicates),
            Err(err) => match err {
                SolutionPredicatesError::Acquire(err) => {
                    return Err(ValidationError::DbPoolClosed(err))
                }
                SolutionPredicatesError::QueryPredicate(addr, err) => match err {
                    QueryPredicateError::Query(err) => return Err(ValidationError::Query(err)),
                    QueryPredicateError::Decode(_)
                    | QueryPredicateError::MissingLenBytes
                    | QueryPredicateError::InvalidLenBytes => {
                        return Ok(ValidateOutcome::Invalid(InvalidOutcome {
                            failure: ValidateFailure::InvalidPredicate(addr),
                            solution_index,
                        }));
                    }
                },
                SolutionPredicatesError::MissingPredicate(addr) => {
                    return Ok(ValidateOutcome::Invalid(InvalidOutcome {
                        failure: ValidateFailure::MissingPredicate(addr),
                        solution_index,
                    }));
                }
            },
        };

        let get_predicate = move |addr: &PredicateAddress| {
            predicates
                .get(addr)
                .cloned()
                .expect("predicate must have been fetched in the previous step")
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
                    let value = query_state(
                        &tx,
                        &contract_addr,
                        &key,
                        block_number,
                        solution_index,
                        pre_state,
                    )?
                    .unwrap_or_default();
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

/// Retrieve all predicates required by the solution.
// TODO: Make proper use of `State`'s connection pool and query predicates in parallel.
async fn query_solution_predicates(
    state: &State,
    contract_registry: &ContentAddress,
    solution_data: &[SolutionData],
) -> Result<HashMap<PredicateAddress, Arc<Predicate>>, SolutionPredicatesError> {
    let mut predicates = HashMap::default();
    let conn = state.conn_pool.acquire().await?;
    for data in solution_data {
        let pred_addr = data.predicate_to_solve.clone();
        let Some(pred) = query_predicate(
            &conn,
            contract_registry,
            &pred_addr,
            state.block_number,
            state.solution_index,
        )
        .map_err(|e| SolutionPredicatesError::QueryPredicate(pred_addr.clone(), e))?
        else {
            return Err(SolutionPredicatesError::MissingPredicate(pred_addr.clone()));
        };
        predicates.insert(pred_addr, Arc::new(pred));
    }
    Ok(predicates)
}

/// Query for the predicate with the given address within state.
///
/// Note that `query_predicate` will always query *inclusive* of the given solution index.
// TODO: Take a connection pool and perform these queries in parallel.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all, err))]
fn query_predicate(
    conn: &rusqlite::Connection,
    contract_registry: &ContentAddress,
    pred_addr: &PredicateAddress,
    block_number: Word,
    solution_ix: u64,
) -> Result<Option<Predicate>, QueryPredicateError> {
    use essential_node_types::contract_registry;
    let pre_state = false;

    #[cfg(feature = "tracing")]
    tracing::trace!("{}:{}", pred_addr.contract, pred_addr.predicate);

    // Check whether the predicate is registered within the associated contract.
    let contract_predicate_key = contract_registry::contract_predicate_key(pred_addr);
    if query_state(
        conn,
        contract_registry,
        &contract_predicate_key,
        block_number,
        solution_ix,
        pre_state,
    )?
    .is_none()
    {
        // If it is not associated with the contract, return `None`.
        return Ok(None);
    }

    // Query the full predicate from the contract registry.
    let predicate_key = contract_registry::predicate_key(&pred_addr.predicate);
    let Some(pred_words) = query_state(
        conn,
        contract_registry,
        &predicate_key,
        block_number,
        solution_ix,
        pre_state,
    )?
    else {
        // If no entry for the predicate, return `None`.
        return Ok(None);
    };

    // Read the length from the front.
    let Some(&pred_len_bytes) = pred_words.first() else {
        return Err(QueryPredicateError::MissingLenBytes);
    };
    let pred_len_bytes: usize = pred_len_bytes
        .try_into()
        .map_err(|_| QueryPredicateError::InvalidLenBytes)?;
    let pred_words = &pred_words[1..];
    let pred_bytes: Vec<u8> = pred_words
        .iter()
        .copied()
        .flat_map(bytes_from_word)
        .take(pred_len_bytes)
        .collect();

    let predicate = Predicate::decode(&pred_bytes)?;
    Ok(Some(predicate))
}

fn query_state(
    conn: &rusqlite::Connection,
    contract_ca: &ContentAddress,
    key: &Key,
    block_number: Word,
    solution_ix: u64,
    pre_state: bool,
) -> Result<Option<Value>, QueryError> {
    if pre_state {
        query_state_exclusive_solution(conn, contract_ca, key, block_number, solution_ix)
    } else {
        query_state_inclusive_solution(conn, contract_ca, key, block_number, solution_ix)
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
