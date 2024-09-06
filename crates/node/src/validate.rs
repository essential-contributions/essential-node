use crate::{
    db::{self, ConnectionPool},
    error::{StateReadError, ValidationError},
};
use essential_check::{
    solution::{check_predicates, CheckPredicateConfig},
    state_read_vm::StateRead,
};
use essential_node_db::{
    finalized::{query_state_exclusive_solution, query_state_inclusive_solution},
    get_predicate,
};
use essential_types::{predicate::Predicate, Block, ContentAddress, Key, PredicateAddress, Word};
use futures::FutureExt;
use std::{
    collections::{HashMap, HashSet},
    pin::Pin,
    sync::Arc,
};

#[derive(Clone)]
struct State {
    block_number: u64,
    solution_index: u64,
    pre_state: bool,
    conn_pool: db::ConnectionPool,
}

/// Validates a block.
///
/// Returns a tuple with a boolean indicating whether the block is valid and
/// an optional hash of the solution that failed validation.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub async fn validate(
    conn_pool: &ConnectionPool,
    block: &Block,
) -> Result<(bool, Option<ContentAddress>), ValidationError> {
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
                    .expect("predicate must have been read in the previous step"),
            )
        };
        if let Err(err) = check_predicates(
            &pre_state,
            &post_state,
            Arc::new(solution.clone()),
            get_predicate,
            Arc::new(CheckPredicateConfig::default()),
        )
        .await
        {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Validation failed for block with number {} and address {} at solution index {} with error {}", 
                block.number,
                essential_hash::content_addr(block),
                solution_index,
                err
            );
            return Ok((false, Some(essential_hash::content_addr(solution))));
        }
    }

    Ok((true, None))
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
