//! # Action
//!
//! Actions use predicates to generate solutions.

use essential_types::{PredicateAddress, Value};

/// Actions represent are used as the input to solve a predicate
/// and generate a [`Solution`](essential_types::Solution).
pub struct Action {
    /// The predicate that will be solved by this action.
    pub predicate_to_solve: PredicateAddress,
    /// The inputs to the predicate.
    pub inputs: Vec<Value>,
}

/// An atomic unordered set of actions.
/// These all occur at the same time.
/// It is invalid for the solutions generated
/// by these actions to mutate the same keys.
pub struct ActionSet {
    /// An unordered set of actions.
    pub actions: Vec<Action>,
}
