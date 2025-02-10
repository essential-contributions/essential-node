//! Shared logic around `essential-check` for validating sequences of solution sets.
//! Used by:
//!
//! - `essential-node` for validation and
//! - `essential-builder` for checking sequencing of solution sets.

#![warn(missing_docs)]

use crate::error::{
    CheckSetError, InvalidSet, PredicateProgramsError, QueryPredicateError, QueryProgramError,
    SolutionSetPredicatesError,
};
use error::{CheckSetsError, QueryStateRangeError};
use essential_check::{
    self as check,
    solution::CheckPredicateConfig,
    types::{
        convert::bytes_from_word, ContentAddress, Key, Predicate, PredicateAddress, Program,
        Solution, SolutionSet, Value, Word,
    },
    vm::{Gas, StateRead},
};
use essential_node_types::{contract_registry, program_registry};
use futures::FutureExt;
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

pub mod error;

/// Query the state of a single key at a particular location within a block.
pub trait QueryStateExcl {
    /// The error returned from `query_state_excl`.
    type Error: std::fmt::Debug + std::fmt::Display;
    /// Query the state prior to the solution set at the given index.
    fn query_state_excl(
        &self,
        contract_ca: &ContentAddress,
        key: &Key,
        state_view_ix: &StateViewIx,
    ) -> impl Future<Output = Result<Option<Value>, Self::Error>> + Send;
}

/// An index representing a view into state at a given block and solution set index.
#[derive(Clone, Debug)]
pub struct StateViewIx {
    /// The block at which we're viewing state.
    pub block: BlockRef,
    /// The solution set index within the block at which we're viewing state.
    pub solution_set_ix: SolutionSetIx,
}

/// A reference to a block, either via a specific block address, or via block number.
#[derive(Clone, Debug)]
pub enum BlockRef {
    /// Refers to a block at a specific address.
    Address(ContentAddress),
    /// Refers to a block at a given block number. Should only be used in the case that the block
    /// number is unambiguous, i.e.
    Number(BlockNum),
}

/// Block number.
pub type BlockNum = Word;

/// The index of a solution set within a sequence of solution sets.
pub type SolutionSetIx = u64;

/// A view into state at a given block and solution set index, providing a StateRead impl.
#[derive(Clone)]
pub struct StateView<T> {
    /// Access to state.
    pub state: T,
    /// The state view index at which we should query state.
    pub ix: StateViewIx,
}

impl<T> StateRead for StateView<T>
where
    T: 'static + Clone + QueryStateExcl + Send,
{
    type Error = QueryStateRangeError<T::Error>;
    type Future = Pin<Box<dyn Future<Output = Result<Vec<Value>, Self::Error>> + Send>>;
    fn key_range(&self, contract: ContentAddress, key: Key, num_values: usize) -> Self::Future {
        let state = self.state.clone();
        let ix = self.ix.clone();
        async move { query_range(state, ix, contract, key, num_values).await }.boxed()
    }
}

/// Check a sequential chunk of solution sets in parallel.
pub async fn check_solution_set_chunk<T>(
    state: T,
    block: BlockRef,
    chunk: impl IntoIterator<Item = (SolutionSetIx, Arc<SolutionSet>)>,
    contract_registry: &ContentAddress,
    program_registry: &ContentAddress,
    check_conf: &Arc<check::solution::CheckPredicateConfig>,
) -> Result<Vec<Result<Gas, InvalidSet<T::Error>>>, CheckSetsError<T::Error>>
where
    T: 'static + Clone + QueryStateExcl + Send + Sync,
    T::Error: 'static + Send,
{
    // Spawn concurrent checks for each solution set.
    let checks: tokio::task::JoinSet<_> = chunk
        .into_iter()
        .map(move |(set_ix, set)| {
            let state = state.clone();
            let check_conf = check_conf.clone();
            let contract_registry = contract_registry.clone();
            let program_registry = program_registry.clone();
            let pre_state_view_ix = StateViewIx {
                block: block.clone(),
                solution_set_ix: set_ix,
            };
            async move {
                let res = check_solution_set(
                    state.clone(),
                    pre_state_view_ix,
                    set.clone(),
                    &contract_registry,
                    &program_registry,
                    check_conf,
                )
                .await;
                (set_ix, res)
            }
        })
        .collect();

    // Await the results.
    let mut results = checks.join_all().await;
    results.sort_by_key(|&(ix, _)| ix);
    results
        .into_iter()
        .map(|(ix, res)| res.map_err(|e| CheckSetsError::CheckSolutionSet(ix, e)))
        .collect()
}

/// Validate the given solution set.
///
/// If the solution set is valid, returns the total gas spent.
pub async fn check_solution_set<T>(
    state: T,
    pre_state_view_ix: StateViewIx,
    solution_set: Arc<SolutionSet>,
    contract_registry: &ContentAddress,
    program_registry: &ContentAddress,
    check_conf: Arc<CheckPredicateConfig>,
) -> Result<Result<Gas, InvalidSet<T::Error>>, CheckSetError<T::Error>>
where
    T: 'static + Clone + QueryStateExcl + Send + Sync,
    T::Error: 'static + Send,
{
    // Prepare the post-state view index.
    let post_state_view_ix = StateViewIx {
        block: pre_state_view_ix.block.clone(),
        solution_set_ix: pre_state_view_ix
            .solution_set_ix
            .checked_add(1)
            .ok_or(CheckSetError::SolutionSetIxOutOfBounds)?,
    };

    // Retrieve the predicates that the solution set attempts to solve from the post-state. This
    // ensures that the solution set has access to contracts submitted as a part of the solution
    // set.
    let predicates = match get_solution_set_predicates(
        state.clone(),
        post_state_view_ix.clone(),
        contract_registry,
        &solution_set.solutions,
    )
    .await
    {
        Ok(predicates) => predicates,
        Err(SolutionSetPredicatesError::PredicateDoesNotExist(ca)) => {
            return Ok(Err(InvalidSet::PredicateDoesNotExist(ca)));
        }
        Err(SolutionSetPredicatesError::QueryPredicate(err)) => match err {
            QueryPredicateError::Decode(_)
            | QueryPredicateError::MissingLenBytes
            | QueryPredicateError::InvalidLenBytes => {
                return Ok(Err(InvalidSet::PredicateInvalid));
            }
            QueryPredicateError::QueryState(err) => return Err(CheckSetError::QueryState(err)),
        },
    };

    // Retrieve the programs that the predicates specify from the post-state.
    let programs = match get_predicates_programs(
        state.clone(),
        post_state_view_ix.clone(),
        program_registry,
        &predicates,
    )
    .await
    {
        Ok(programs) => programs,
        Err(PredicateProgramsError::ProgramDoesNotExist(ca)) => {
            return Ok(Err(InvalidSet::ProgramDoesNotExist(ca)));
        }
        Err(PredicateProgramsError::QueryProgram(err)) => match err {
            QueryProgramError::MissingLenBytes | QueryProgramError::InvalidLenBytes => {
                return Ok(Err(InvalidSet::ProgramInvalid));
            }
            QueryProgramError::QueryState(err) => return Err(CheckSetError::QueryState(err)),
        },
    };

    let get_predicate = move |addr: &PredicateAddress| {
        predicates
            .get(&addr.predicate)
            .cloned()
            .expect("predicate must have been fetched in the previous step")
    };

    let get_program = move |addr: &ContentAddress| {
        programs
            .get(addr)
            .cloned()
            .expect("program must have been fetched in the previous step")
    };

    let pre_state = StateView {
        state: state.clone(),
        ix: pre_state_view_ix,
    };
    let post_state = StateView {
        state,
        ix: post_state_view_ix,
    };

    // Create the post-state and check the solution set's predicates.
    match check::solution::check_set_predicates(
        &pre_state,
        &post_state,
        solution_set.clone(),
        get_predicate,
        get_program,
        check_conf.clone(),
    )
    .await
    {
        Err(err) => Ok(Err(InvalidSet::Predicates(err))),
        Ok(gas) => Ok(Ok(gas)),
    }
}

/// Read and return all predicates required by the given set of solutions.
pub async fn get_solution_set_predicates<T>(
    state_view: T,
    state_view_ix: StateViewIx,
    contract_registry: &ContentAddress,
    solutions: &[Solution],
) -> Result<HashMap<ContentAddress, Arc<Predicate>>, SolutionSetPredicatesError<T::Error>>
where
    T: 'static + Clone + QueryStateExcl + Send,
    T::Error: 'static + Send,
{
    // Spawn concurrent queries for each predicate.
    let queries: tokio::task::JoinSet<_> = solutions
        .iter()
        .map(|solution| solution.predicate_to_solve.clone())
        .enumerate()
        .map(move |(ix, pred_addr)| {
            let view = state_view.clone();
            let view_ix = state_view_ix.clone();
            let registry = contract_registry.clone();
            async move {
                let pred = get_predicate(view, &view_ix, &registry, &pred_addr).await;
                (ix, pred)
            }
        })
        .collect();

    // Collect the results into a map.
    let mut map = HashMap::new();
    let mut results = queries.join_all().await;
    results.sort_by_key(|(ix, _)| *ix);
    for (sol, (_ix, res)) in solutions.iter().zip(results) {
        let ca = sol.predicate_to_solve.predicate.clone();
        let predicate =
            res?.ok_or_else(|| SolutionSetPredicatesError::PredicateDoesNotExist(ca.clone()))?;
        map.insert(ca, Arc::new(predicate));
    }

    Ok(map)
}

/// Read and return all programs required by the given predicates.
pub async fn get_predicates_programs<T>(
    state_view: T,
    state_view_ix: StateViewIx,
    program_registry: &ContentAddress,
    predicates: &HashMap<ContentAddress, Arc<Predicate>>,
) -> Result<HashMap<ContentAddress, Arc<Program>>, PredicateProgramsError<T::Error>>
where
    T: 'static + Clone + QueryStateExcl + Send,
    T::Error: 'static + Send,
{
    // Spawn concurrent queries for each program.
    let queries: tokio::task::JoinSet<_> = predicates
        .iter()
        .flat_map(|(_, pred)| {
            let view = state_view.clone();
            let view_ix = state_view_ix.clone();
            pred.nodes
                .iter()
                .map(|node| node.program_address.clone())
                .enumerate()
                .map(move |(ix, prog_addr)| {
                    let view = view.clone();
                    let view_ix = view_ix.clone();
                    let registry = program_registry.clone();
                    async move {
                        let prog = get_program(view, &view_ix, &registry, &prog_addr).await;
                        (ix, prog)
                    }
                })
        })
        .collect();

    // Collect the results into a map.
    let mut map = HashMap::new();
    let mut results = queries.join_all().await;
    results.sort_by_key(|(ix, _)| *ix);

    for (node, (_ix, res)) in predicates
        .iter()
        .flat_map(|(_, pred)| pred.nodes.iter())
        .zip(results)
    {
        let ca = node.program_address.clone();
        let program =
            res?.ok_or_else(|| PredicateProgramsError::ProgramDoesNotExist(ca.clone()))?;
        map.insert(ca, Arc::new(program));
    }

    Ok(map)
}

/// Get the predicate at the given content address.
pub(crate) async fn get_predicate<T>(
    state_view: T,
    state_view_ix: &StateViewIx,
    contract_registry: &ContentAddress,
    pred_addr: &PredicateAddress,
) -> Result<Option<Predicate>, QueryPredicateError<T::Error>>
where
    T: QueryStateExcl,
{
    // Check that the predicate is a part of the contract.
    let contract_predicate_key = contract_registry::contract_predicate_key(pred_addr);
    if state_view
        .query_state_excl(contract_registry, &contract_predicate_key, state_view_ix)
        .await
        .map_err(QueryPredicateError::QueryState)?
        .is_none()
    {
        return Ok(None);
    }

    // Read the full predicate out of the contract registry storage.
    let predicate_key = contract_registry::predicate_key(&pred_addr.predicate);
    let Some(pred_words) = state_view
        .query_state_excl(contract_registry, &predicate_key, state_view_ix)
        .await
        .map_err(QueryPredicateError::QueryState)?
    else {
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

/// Get the program at the given content address.
pub(crate) async fn get_program<T>(
    state_view: T,
    state_view_ix: &StateViewIx,
    program_registry: &ContentAddress,
    prog_addr: &ContentAddress,
) -> Result<Option<Program>, QueryProgramError<T::Error>>
where
    T: QueryStateExcl,
{
    let program_key = program_registry::program_key(prog_addr);
    let Some(prog_words) = state_view
        .query_state_excl(program_registry, &program_key, state_view_ix)
        .await
        .map_err(QueryProgramError::QueryState)?
    else {
        return Ok(None);
    };

    // Read the length from the front.
    let Some(&prog_len_bytes) = prog_words.first() else {
        return Err(QueryProgramError::MissingLenBytes);
    };
    let prog_len_bytes: usize = prog_len_bytes
        .try_into()
        .map_err(|_| QueryProgramError::InvalidLenBytes)?;
    let prog_words = &prog_words[1..];
    let prog_bytes: Vec<u8> = prog_words
        .iter()
        .copied()
        .flat_map(bytes_from_word)
        .take(prog_len_bytes)
        .collect();

    let program = Program(prog_bytes);
    Ok(Some(program))
}

/// Query a range of keys and return the resulting state.
async fn query_range<T>(
    state: T,
    state_view_ix: StateViewIx,
    contract_ca: ContentAddress,
    mut key: Key,
    mut num_values: usize,
) -> Result<Vec<Value>, QueryStateRangeError<T::Error>>
where
    T: QueryStateExcl,
{
    let mut values = vec![];
    while num_values > 0 {
        let value = state
            .query_state_excl(&contract_ca, &key, &state_view_ix)
            .await
            .map_err(QueryStateRangeError::QueryState)?
            .unwrap_or(vec![]);
        values.push(value);
        key = next_key(key).map_err(|key| QueryStateRangeError::OutOfRange { key, num_values })?;
        num_values -= 1;
    }
    Ok(values)
}

/// Calculate the next key.
fn next_key(mut key: Key) -> Result<Key, Key> {
    for w in key.iter_mut().rev() {
        match *w {
            Word::MAX => *w = Word::MIN,
            _ => {
                *w += 1;
                return Ok(key);
            }
        }
    }
    Err(key)
}
