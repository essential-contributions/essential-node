use blake2b_rs::{Blake2b, Blake2bBuilder};
use essential_types::{solution::Mutation, ContentAddress, Key, SolutionSet, Value, Word};
use sparse_merkle_tree::{
    blake2b::Blake2bHasher, default_store::DefaultStore, SparseMerkleTree, H256,
};
use std::collections::HashMap;

#[cfg(test)]
mod tests;

// TODO: chose between HashMap & BTreeMap. Is State already defined?
#[derive(Clone, Debug)]
pub struct State(HashMap<ContentAddress, HashMap<Key, Vec<Word>>>);

// define SMT
type SMT = SparseMerkleTree<Blake2bHasher, Word, DefaultStore<Word>>;

impl State {
    // Empry state, fine for tests unrelated to reading state.
    pub fn empty() -> Self {
        State(HashMap::new())
    }
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

// Expose a function that takes a solution set and a State, and applies it to the state.
pub fn apply_solution_set(set: &SolutionSet, state: &mut State) -> State {
    for solution in set.solutions.iter() {
        let contract_address = &solution.predicate_to_solve.contract;

        // Get or create the inner HashMap for the contract address
        let inner_map = state
            .0
            .entry(contract_address.clone())
            .or_insert_with(HashMap::new);

        // Borrow `state_mutations` to avoid moving it
        for mutation in &solution.state_mutations {
            inner_map.insert(mutation.key.clone(), mutation.value.clone());
        }
    }
    state.clone()
}

// Expose a function which takes a State and returns a merkle tree
fn generate_merkle_tree(state: &State) -> SMT {
    // TODO: chose hashing algorithm to use.
    // TODO: look into how we generate a MerkleTree from a hashmap.
    SMT::default()
}
