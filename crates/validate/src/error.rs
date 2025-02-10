//! Error types for `essential-validate`.

use essential_check::{
    solution::PredicatesError,
    types::{predicate::PredicateDecodeError, ContentAddress, Key},
};
use thiserror::Error;

use crate::SolutionSetIx;

/// Any errors that might occur within `check_sets`.
#[derive(Debug, Error)]
pub enum CheckSetsError<E> {
    /// An error occurred while checking a solution set.
    #[error("an error occurred while attempting to check set {0}: {1}")]
    CheckSolutionSet(SolutionSetIx, CheckSetError<E>),
}

/// Any errors that might occur during [`check_set`][crate::check_set
#[derive(Debug, Error)]
pub enum CheckSetError<E> {
    /// The given pre-state solution set index would imply a post-state solution set index that
    /// would exceed `u64::MAX`.
    #[error("the post-state solution set index would exceed `u64::MAX`")]
    SolutionSetIxOutOfBounds,
    /// Failed to query state.
    #[error("failed to query state: {0}")]
    QueryState(E),
}

/// Represents the reason why a [`SolutionSet`][essential_types::solution::SolutionSet] is invalid.
#[derive(Debug, Error)]
pub enum InvalidSet<E> {
    /// Solution set specified a predicate to solve that does not exist.
    #[error("Solution set specified a predicate to solve that does not exist")]
    PredicateDoesNotExist(ContentAddress),
    /// Solution set contains a predicate that specified a program that does not exist.
    #[error("Solution set contains a predicate that specified a program that does not exist")]
    ProgramDoesNotExist(ContentAddress),
    /// Solution set specified a predicate that exists, but was invalid when reading from contract
    /// registry state.
    #[error(
        "Solution set specified a predicate that was invalid when reading from contract registry state"
    )]
    PredicateInvalid,
    /// Solution set contains a predicate that specified a program that exists,
    /// but was invalid when reading from program registry state.
    #[error(
        "Solution set contains a predicate that specified a program that was invalid when reading from program registry state"
    )]
    ProgramInvalid,
    /// Validation of the solution set predicates failed.
    #[error("Validation of the solution set predicates failed: {0}")]
    Predicates(PredicatesError<QueryStateRangeError<E>>),
}

/// An error occurred while fetching a solution set's predicates.
#[derive(Debug, Error)]
pub enum SolutionSetPredicatesError<E> {
    /// An error occurred while querying for a predicate.
    #[error("an error occurred while querying for a predicate: {0}")]
    QueryPredicate(#[from] QueryPredicateError<E>),
    /// The required predicate does not exist.
    #[error("the required predicate ({0}) does not exist")]
    PredicateDoesNotExist(ContentAddress),
}

/// Any errors that might occur while querying for predicates.
#[derive(Debug, Error)]
pub enum QueryPredicateError<E> {
    /// Failed to query state.
    #[error("failed to query state: {0}")]
    QueryState(E),
    /// The queried predicate is missing the word that encodes its length.
    #[error("the queried predicate is missing the word that encodes its length")]
    MissingLenBytes,
    /// The queried predicate length was invalid.
    #[error("the queried predicate length was invalid")]
    InvalidLenBytes,
    /// Failed to decode the queried predicate.
    #[error("failed to decode the queried predicate: {0}")]
    Decode(#[from] PredicateDecodeError),
}

/// An error occurred while fetching a predicate's programs.
#[derive(Debug, Error)]
pub enum PredicateProgramsError<E> {
    /// An error occurred while querying the node DB.
    #[error("an error occurred while querying for a program from the node DB: {0}")]
    QueryProgram(#[from] QueryProgramError<E>),
    /// The node DB is missing a required predicate.
    #[error("the node DB is missing a required program ({0})")]
    ProgramDoesNotExist(ContentAddress),
}

/// Any errors that might occur while querying for programs.
#[derive(Debug, Error)]
pub enum QueryProgramError<E> {
    /// Failed to query state.
    #[error("failed to query state: {0}")]
    QueryState(E),
    /// The queried program is missing the word that encodes its length.
    #[error("the queried program is missing the word that encodes its length")]
    MissingLenBytes,
    /// The queried program length was invalid.
    #[error("the queried program length was invalid")]
    InvalidLenBytes,
}

/// Any errors that might occur while querying a range of keys.
#[derive(Debug, Error)]
pub enum QueryStateRangeError<E> {
    /// Failed to query state.
    #[error("failed to query state: {0}")]
    QueryState(E),
    /// Key out of range.
    #[error("A key would be out of range: `key` {key:?}, `num_values` {num_values}")]
    OutOfRange {
        /// The initial key of the state range query.
        key: Key,
        /// the total number of values requested.
        num_values: usize,
    },
}
