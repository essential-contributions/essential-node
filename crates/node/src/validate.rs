//! # Validation
//! Functions for validating blocks and solutions.
use crate::{
    db::{
        self,
        finalized::{query_state_exclusive_solution_set, query_state_inclusive_solution_set},
        pool::ConnectionHandle,
        ConnectionPool, QueryError,
    },
    error::{
        PredicatesProgramsError, QueryPredicateError, QueryProgramError,
        SolutionSetPredicatesError, StateReadError, ValidationError,
    },
};
use essential_check::{
    solution::{check_set_predicates, CheckPredicateConfig, PredicatesError},
    vm::{Gas, StateRead},
};
use essential_node_types::{Block, BlockHeader};
use essential_types::{
    convert::bytes_from_word,
    predicate::{Predicate, Program},
    solution::{Solution, SolutionSet},
    ContentAddress, Key, PredicateAddress, Value, Word,
};
use futures::FutureExt;
use std::{collections::HashMap, pin::Pin, sync::Arc};

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct State {
    block_number: Word,
    solution_set_index: u64,
    pre_state: bool,
    conn_pool: Db,
}

#[derive(Clone)]
/// Either a dry run database or a connection pool.
enum Db {
    DryRun(DryRun),
    ConnectionPool(ConnectionPool),
}

#[derive(Clone)]
/// A dry run database.
///
/// Cascades from in-memory to on-disk database.
struct DryRun {
    memory: Memory,
    conn_pool: ConnectionPool,
}

#[derive(Clone)]
/// In-memory database that contains a dry run block.
struct Memory(db::ConnectionPool);

/// Either a cascading handle or a connection handle.
enum Conn {
    Cascade(Cascade),
    Handle(ConnectionHandle),
}

/// Either a cascading transaction or a transaction.
enum Transaction<'a> {
    Cascade(CascadeTransaction<'a>),
    Handle(rusqlite::Transaction<'a>),
}

/// Cascading handle that cascades from in-memory to on-disk database.
struct Cascade {
    memory: ConnectionHandle,
    db: ConnectionHandle,
}

/// Cascading transaction that cascades from in-memory to on-disk database.
struct CascadeTransaction<'a> {
    memory: rusqlite::Transaction<'a>,
    db: rusqlite::Transaction<'a>,
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
    /// The index of the solution set that caused the failure.
    pub solution_set_index: usize,
}

/// Reasons for a block to be invalid.
/// Contains the error that caused the block to be invalid.
#[derive(Debug)]
pub enum ValidateFailure {
    /// A solution specified a predicate that does not exist within the contract registry.
    MissingPredicate(PredicateAddress),
    /// A predicate was present in the registry, but failed to decode.
    InvalidPredicate(PredicateAddress),
    /// A predicate specified a program that does not exist within the program registry.
    MissingProgram(ContentAddress),
    /// A program was present in the registry, but has an invalid format.
    InvalidProgram(ContentAddress),
    #[allow(dead_code)]
    /// A predicate failed to validate.
    PredicatesError(PredicatesError<StateReadError>),
    /// The total gas consumed by all solutions in the block exceeds the maximum gas limit.
    GasOverflow,
}

/// Validates a solution without adding it to the database.
/// Creates a block at the next block number and current timestamp with the given solution set
/// and validates it.
///
/// Returns a `ValidationResult` if no `ValidationError` occurred that prevented the solution set from being validated.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub async fn validate_solution_set_dry_run(
    conn_pool: &ConnectionPool,
    contract_registry: &ContentAddress,
    program_registry: &ContentAddress,
    solution_set: SolutionSet,
) -> Result<ValidateOutcome, ValidationError> {
    let mut conn = conn_pool.acquire().await?;
    let tx = conn.transaction()?;
    let number = match db::get_latest_finalized_block_address(&tx)? {
        Some(address) => db::get_block_header(&tx, &address)?
            .map(|header| header.number)
            .unwrap_or(1),
        None => 1,
    };
    let block = Block {
        header: BlockHeader {
            number,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time must be valid"),
        },
        solution_sets: vec![solution_set],
    };
    drop(tx);
    validate_dry_run(conn_pool, contract_registry, program_registry, &block).await
}

/// Validates a block without adding the block to the database.
///
/// Returns a `ValidationResult` if no `ValidationError` occurred that prevented the block from being validated.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub async fn validate_dry_run(
    conn_pool: &ConnectionPool,
    contract_registry: &ContentAddress,
    program_registry: &ContentAddress,
    block: &Block,
) -> Result<ValidateOutcome, ValidationError> {
    let dry_run = DryRun::new(conn_pool.clone(), block).await?;
    let db_type = Db::DryRun(dry_run);
    validate_inner(db_type, contract_registry, program_registry, block).await
}

/// Validates a block.
///
/// Returns a `ValidationResult` if no `ValidationError` occurred that prevented the block from being validated.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub(crate) async fn validate(
    conn_pool: &ConnectionPool,
    contract_registry: &ContentAddress,
    program_registry: &ContentAddress,
    block: &Block,
) -> Result<ValidateOutcome, ValidationError> {
    let db_type = Db::ConnectionPool(conn_pool.clone());
    validate_block(db_type.clone(), block).await?;
    validate_inner(db_type, contract_registry, program_registry, block).await
}

/// Validates a block.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
async fn validate_block(_conn: Db, _block: &Block) -> Result<(), ValidationError> {
    todo!()
}

/// Validates a block's solutions.
///
/// Returns a `ValidationResult` if no `ValidationError` occurred that prevented the block from being validated.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
async fn validate_inner(
    conn: Db,
    contract_registry: &ContentAddress,
    program_registry: &ContentAddress,
    block: &Block,
) -> Result<ValidateOutcome, ValidationError> {
    let mut total_gas: u64 = 0;

    // Check predicates and programs.
    for (solution_set_index, solution_set) in block.solution_sets.iter().enumerate() {
        let pre_state = State {
            block_number: block.header.number,
            solution_set_index: solution_set_index as u64,
            pre_state: true,
            conn_pool: conn.clone(),
        };
        let post_state = State {
            block_number: block.header.number,
            solution_set_index: solution_set_index as u64,
            pre_state: false,
            conn_pool: conn.clone(),
        };

        // Create the `predicates` map.
        let res =
            query_solution_set_predicates(&post_state, contract_registry, &solution_set.solutions)
                .await;
        let predicates = match res {
            Ok(predicates) => Arc::new(predicates),
            Err(err) => match err {
                SolutionSetPredicatesError::Acquire(err) => {
                    return Err(ValidationError::DbPoolClosed(err))
                }
                SolutionSetPredicatesError::QueryPredicate(addr, err) => match err {
                    QueryPredicateError::Query(err) => return Err(ValidationError::Query(err)),
                    QueryPredicateError::Decode(_)
                    | QueryPredicateError::MissingLenBytes
                    | QueryPredicateError::InvalidLenBytes => {
                        return Ok(ValidateOutcome::Invalid(InvalidOutcome {
                            failure: ValidateFailure::InvalidPredicate(addr),
                            solution_set_index,
                        }));
                    }
                },
                SolutionSetPredicatesError::MissingPredicate(addr) => {
                    return Ok(ValidateOutcome::Invalid(InvalidOutcome {
                        failure: ValidateFailure::MissingPredicate(addr),
                        solution_set_index,
                    }));
                }
            },
        };

        // Create the `programs` map.
        let res = query_predicates_programs(&post_state, program_registry, &predicates).await;
        let programs = match res {
            Ok(programs) => Arc::new(programs),
            Err(err) => match err {
                PredicatesProgramsError::Acquire(err) => {
                    return Err(ValidationError::DbPoolClosed(err))
                }
                PredicatesProgramsError::QueryProgram(addr, err) => match err {
                    QueryProgramError::Query(err) => return Err(ValidationError::Query(err)),
                    QueryProgramError::MissingLenBytes | QueryProgramError::InvalidLenBytes => {
                        return Ok(ValidateOutcome::Invalid(InvalidOutcome {
                            failure: ValidateFailure::InvalidProgram(addr),
                            solution_set_index,
                        }));
                    }
                },
                PredicatesProgramsError::MissingProgram(addr) => {
                    return Ok(ValidateOutcome::Invalid(InvalidOutcome {
                        failure: ValidateFailure::MissingProgram(addr),
                        solution_set_index,
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

        let get_program = move |addr: &ContentAddress| {
            programs
                .get(addr)
                .cloned()
                .expect("program must have been fetched in the previous step")
        };

        match check_set_predicates(
            &pre_state,
            &post_state,
            Arc::new(solution_set.clone()),
            get_predicate,
            get_program,
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
                        solution_set_index,
                    }));
                }
            }
            Err(err) => {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Validation failed for block with number {} and address {} at solution set index {} with error {}",
                    block.header.number,
                    essential_hash::content_addr(block),
                    solution_set_index,
                    err
                );
                return Ok(ValidateOutcome::Invalid(InvalidOutcome {
                    failure: ValidateFailure::PredicatesError(err),
                    solution_set_index,
                }));
            }
        }
    }

    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Validation successful for block with number {} and address {}. Gas: {}",
        block.header.number,
        essential_hash::content_addr(block),
        total_gas
    );
    Ok(ValidateOutcome::Valid(ValidOutcome { total_gas }))
}

impl DryRun {
    /// Create a new dry run database which puts the given block into memory
    /// then cascades from in-memory to on-disk database.
    pub async fn new(conn_pool: ConnectionPool, block: &Block) -> Result<Self, rusqlite::Error> {
        let memory = Memory::new(block)?;
        Ok(Self { memory, conn_pool })
    }
}

impl Memory {
    /// Create a new in-memory database with the given block.
    fn new(block: &Block) -> Result<Self, rusqlite::Error> {
        // Only need one connection for the memory database
        // as there is no contention.
        let config = db::pool::Config {
            conn_limit: 1,
            source: db::pool::Source::Memory(uuid::Uuid::new_v4().to_string()),
        };
        let memory = db::ConnectionPool::new(&config)?;
        let mut conn = memory
            .try_acquire()
            .expect("can't fail due to no other connections");

        // Insert and finalize the block.
        let tx = conn.transaction()?;
        essential_node_db::create_tables(&tx)?;
        let hash = essential_node_db::insert_block(&tx, block)?;
        essential_node_db::finalize_block(&tx, &hash)?;
        tx.commit()?;

        Ok(Self(memory))
    }
}

impl Db {
    /// Acquire a connection from the database.
    pub async fn acquire(&self) -> Result<Conn, tokio::sync::AcquireError> {
        let conn = match self {
            Db::DryRun(dry_run) => {
                let cascade = Cascade {
                    memory: dry_run.memory.as_ref().acquire().await?,
                    db: dry_run.conn_pool.acquire().await?,
                };
                Conn::Cascade(cascade)
            }
            Db::ConnectionPool(conn_pool) => Conn::Handle(conn_pool.acquire().await?),
        };
        Ok(conn)
    }
}

impl Conn {
    /// Start a transaction.
    fn transaction(&mut self) -> Result<Transaction<'_>, rusqlite::Error> {
        match self {
            Conn::Cascade(cascade) => {
                let memory = cascade.memory.transaction()?;
                let db = cascade.db.transaction()?;
                Ok(Transaction::Cascade(CascadeTransaction { memory, db }))
            }
            Conn::Handle(handle) => {
                let tx = handle.transaction()?;
                Ok(Transaction::Handle(tx))
            }
        }
    }
}

/// Cascade from in-memory to on-disk database across transactions.
fn cascade(
    conn: &CascadeTransaction,
    f: impl Fn(&rusqlite::Transaction) -> Result<Option<Value>, QueryError>,
) -> Result<Option<Value>, QueryError> {
    match f(&conn.memory)? {
        Some(val) => Ok(Some(val)),
        None => f(&conn.db),
    }
}

/// Run a query on either a cascade or a handle.
fn query(
    conn: &Transaction,
    f: impl Fn(&rusqlite::Transaction) -> Result<Option<Value>, QueryError>,
) -> Result<Option<Value>, QueryError> {
    match conn {
        Transaction::Cascade(cascade_tx) => cascade(cascade_tx, f),
        Transaction::Handle(tx) => f(tx),
    }
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
            solution_set_index,
            pre_state,
            conn_pool,
        } = self.clone();

        async move {
            let mut conn = conn_pool.acquire().await?;

            tokio::task::spawn_blocking(move || {
                let mut values = vec![];
                let tx = conn.transaction()?;

                for _ in 0..num_values {
                    let value = query(&tx, |tx| {
                        query_state(
                            tx,
                            &contract_addr,
                            &key,
                            block_number,
                            solution_set_index,
                            pre_state,
                        )
                    })?;
                    let value = value.unwrap_or_default();
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

/// Retrieve all predicates required by the solution set.
// TODO: Make proper use of `State`'s connection pool and query predicates in parallel.
async fn query_solution_set_predicates(
    state: &State,
    contract_registry: &ContentAddress,
    solutions: &[Solution],
) -> Result<HashMap<PredicateAddress, Arc<Predicate>>, SolutionSetPredicatesError> {
    let mut predicates = HashMap::default();
    let mut conn = state.conn_pool.acquire().await?;
    for solution in solutions {
        let pred_addr = solution.predicate_to_solve.clone();
        let Some(pred) = query_predicate(
            &mut conn,
            contract_registry,
            &pred_addr,
            state.block_number,
            state.solution_set_index,
        )
        .map_err(|e| SolutionSetPredicatesError::QueryPredicate(pred_addr.clone(), e))?
        else {
            return Err(SolutionSetPredicatesError::MissingPredicate(
                pred_addr.clone(),
            ));
        };
        predicates.insert(pred_addr, Arc::new(pred));
    }
    Ok(predicates)
}

/// Query for the predicate with the given address within state.
///
/// Note that `query_predicate` will always query *inclusive* of the given solution set index.
// TODO: Take a connection pool and perform these queries in parallel.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all, err))]
fn query_predicate(
    conn: &mut Conn,
    contract_registry: &ContentAddress,
    pred_addr: &PredicateAddress,
    block_number: Word,
    solution_set_ix: u64,
) -> Result<Option<Predicate>, QueryPredicateError> {
    use essential_node_types::contract_registry;
    let pre_state = false;

    #[cfg(feature = "tracing")]
    tracing::trace!("{}:{}", pred_addr.contract, pred_addr.predicate);

    // Check whether the predicate is registered within the associated contract.
    let contract_predicate_key = contract_registry::contract_predicate_key(pred_addr);
    let tx = conn.transaction().map_err(QueryError::Rusqlite)?;
    if query(&tx, |tx| {
        query_state(
            tx,
            contract_registry,
            &contract_predicate_key,
            block_number,
            solution_set_ix,
            pre_state,
        )
    })?
    .is_none()
    {
        // If it is not associated with the contract, return `None`.
        return Ok(None);
    }

    // Query the full predicate from the contract registry.
    let predicate_key = contract_registry::predicate_key(&pred_addr.predicate);
    let Some(pred_words) = query(&tx, |tx| {
        query_state(
            tx,
            contract_registry,
            &predicate_key,
            block_number,
            solution_set_ix,
            pre_state,
        )
    })?
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

/// Retrieve all programs required by the predicates.
// TODO: Make proper use of `State`'s connection pool and query programs in parallel.
async fn query_predicates_programs(
    state: &State,
    program_registry: &ContentAddress,
    predicates: &HashMap<PredicateAddress, Arc<Predicate>>,
) -> Result<HashMap<ContentAddress, Arc<Program>>, PredicatesProgramsError> {
    let mut programs = HashMap::default();
    let mut conn = state.conn_pool.acquire().await?;
    for predicate in predicates.values() {
        for node in &predicate.nodes {
            let prog_addr = node.program_address.clone();
            let Some(prog) = query_program(
                &mut conn,
                program_registry,
                &prog_addr,
                state.block_number,
                state.solution_set_index,
            )
            .map_err(|e| PredicatesProgramsError::QueryProgram(prog_addr.clone(), e))?
            else {
                return Err(PredicatesProgramsError::MissingProgram(prog_addr.clone()));
            };
            programs.insert(prog_addr, Arc::new(prog));
        }
    }
    Ok(programs)
}

/// Query for the program with the given address within state.
///
/// Note that `query_program` will always query *inclusive* of the given solution set index.
// TODO: Take a connection pool and perform these queries in parallel.
fn query_program(
    conn: &mut Conn,
    program_registry: &ContentAddress,
    prog_addr: &ContentAddress,
    block_number: Word,
    solution_set_ix: u64,
) -> Result<Option<Program>, QueryProgramError> {
    use essential_node_types::program_registry;
    let pre_state = false;

    #[cfg(feature = "tracing")]
    tracing::trace!("{}", prog_addr);

    // Check whether the program is registered within the program registry.
    let program_key = program_registry::program_key(prog_addr);
    let tx = conn.transaction().map_err(QueryError::Rusqlite)?;
    let Some(prog_words) = query(&tx, |tx| {
        query_state(
            tx,
            program_registry,
            &program_key,
            block_number,
            solution_set_ix,
            pre_state,
        )
    })?
    else {
        // If no entry for the program, return `None`.
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

fn query_state(
    conn: &rusqlite::Connection,
    contract_ca: &ContentAddress,
    key: &Key,
    block_number: Word,
    solution_set_ix: u64,
    pre_state: bool,
) -> Result<Option<Value>, QueryError> {
    if pre_state {
        query_state_exclusive_solution_set(conn, contract_ca, key, block_number, solution_set_ix)
    } else {
        query_state_inclusive_solution_set(conn, contract_ca, key, block_number, solution_set_ix)
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

impl AsRef<db::ConnectionPool> for Memory {
    fn as_ref(&self) -> &db::ConnectionPool {
        &self.0
    }
}
