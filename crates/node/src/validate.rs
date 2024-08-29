use crate::{
    db::{self, ConnectionPool},
    error::{RecoverableError, ValidationError},
};
use essential_check::{
    solution::{check_predicates, CheckPredicateConfig},
    state_read_vm::StateRead,
};
use essential_node_db::{
    finalized::{query_state_exclusive_solution, query_state_inclusive_solution},
    get_predicate, QueryError,
};
use essential_types::{predicate::Predicate, Block, ContentAddress, Key, PredicateAddress, Word};
use futures::FutureExt;
use std::{collections::HashMap, pin::Pin, sync::Arc};

#[derive(Clone)]
struct State {
    block_number: u64,
    solution_index: u64,
    pre_state: bool,
    conn_pool: db::ConnectionPool,
}

pub async fn validate(
    db: ConnectionPool,
    private_db: ConnectionPool,
    block: &Block,
) -> Result<(), RecoverableError> {
    let mut predicates: HashMap<PredicateAddress, Predicate> = HashMap::new();

    // TODO: read predicates in one go with a tx

    for (solution_index, solution) in block.solutions.iter().enumerate() {
        for data in &solution.data {
            // Read predicates from database.
            let db = db.clone();
            let conn = db.acquire().await.unwrap();
            let predicate_addr = data.predicate_to_solve.clone();
            if !(predicates.contains_key(&predicate_addr)) {
                let res = tokio::task::spawn_blocking(move || {
                    get_predicate(&conn, &predicate_addr.predicate)
                })
                .await
                .map_err(RecoverableError::Join)?;

                match res {
                    Ok(predicate) => match predicate {
                        Some(p) => {
                            predicates.insert(data.predicate_to_solve.clone(), p);
                        }
                        None => {
                            return Err(RecoverableError::Validation(
                                ValidationError::PredicateNotFound(data.predicate_to_solve.clone()),
                            ));
                        }
                    },
                    Err(err) => {
                        return Err(RecoverableError::Validation(ValidationError::Query(err)));
                    }
                }
            }

            // Check predicates.

            let pre_state = State {
                block_number: block.number,
                solution_index: solution_index as u64,
                pre_state: true,
                conn_pool: private_db.clone(),
            };

            let post_state = State {
                block_number: block.number,
                solution_index: solution_index as u64,
                pre_state: false,
                conn_pool: private_db.clone(),
            };

            let get_predicate =
                |addr: &PredicateAddress| Arc::new(predicates.get(addr).cloned().unwrap());

            let res = check_predicates(
                &pre_state,
                &post_state,
                Arc::new(solution.clone()),
                get_predicate,
                Arc::new(CheckPredicateConfig::default()),
            )
            .await;

            if let Err(_err) = res {
                // TODO: write to db as failed block
                todo!();
            }
        }
    }

    Ok(())
}

impl StateRead for State {
    type Error = QueryError;

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
            let mut conn = conn_pool.acquire().await.unwrap();

            tokio::task::spawn_blocking(move || {
                let tx = conn.transaction().unwrap(); // TODO: don't
                let mut values = vec![];

                for _ in 0..num_values {
                    if pre_state {
                        // TODO: maybe map instead
                        match query_state_exclusive_solution(
                            &tx,
                            &contract_addr,
                            &key,
                            block_number,
                            solution_index,
                        )
                        .unwrap()
                        {
                            Some(value) => values.push(value),
                            None => values.push(vec![]),
                        }
                    } else {
                        // TODO: maybe map instead
                        match query_state_inclusive_solution(
                            &tx,
                            &contract_addr,
                            &key,
                            block_number,
                            solution_index,
                        )
                        .unwrap()
                        {
                            Some(value) => values.push(value),
                            None => values.push(vec![]),
                        }
                    };

                    // TODO: handle error
                    key = next_key(key).unwrap();
                }
                Ok(values)
            })
            .await
            // TODO: handle error
            .unwrap()
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
