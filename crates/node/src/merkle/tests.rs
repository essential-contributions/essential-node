use super::*;
use essential_types::{
    solution::Mutation, ContentAddress, PredicateAddress, Solution, SolutionSet, Word,
};
use sparse_merkle_tree::{
    blake2b::Blake2bHasher, default_store::DefaultStore, SparseMerkleTree, H256,
};

fn test_mutation(salt: usize, val: Word) -> Mutation {
    Mutation {
        key: vec![salt as Word; 4],
        value: vec![val],
    }
}

#[test]
fn test_apply_solution_set() {
    let solution = Solution {
        predicate_to_solve: PredicateAddress {
            contract: ContentAddress([0u8; 32]),
            predicate: ContentAddress([0u8; 32]),
        },
        predicate_data: vec![],
        state_mutations: vec![test_mutation(0, 42)],
    };
    let solution_set = SolutionSet {
        solutions: vec![solution.clone()],
    };
    let mut state = State::empty();

    let mut expected_state = state.clone();
    let inner_map = expected_state
        .0
        .entry(solution.predicate_to_solve.contract)
        .or_insert(HashMap::new());

    inner_map.insert(
        solution.state_mutations[0].key.clone(),
        solution.state_mutations[0].value.clone(),
    );

    let updated_state = apply_solution_set(&solution_set, &mut state);

    assert_eq!(updated_state, expected_state);
}

#[test]
fn test_generate_merkle_tree() {
    // Create a mock state (key-value pairs)
    let key1 = H256::from([0u8; 32]);
    let value = H256::from([1u8; 32]);
    let key2 = H256::from([2u8; 32]);
    let value2 = H256::from([3u8; 32]);
    let state = State::empty();

    let mut smt = SMT::default();

    // Insert key-value pairs into the tree
    // smt.update(key1, value1).unwrap();
    // smt.update(key2, value2).unwrap();

    // Generate the expected root hash manually
    let expected_root = smt.root();

    // Call your function to generate the Merkle tree
    let generated_merkle_tree = generate_merkle_tree(&state);

    // Assert that the generated Merkle tree's root matches the expected root
    assert_eq!(generated_merkle_tree.root(), expected_root);
}
