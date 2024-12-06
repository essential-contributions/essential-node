use super::*;
use essential_types::{ContentAddress, PredicateAddress, Solution};

#[test]
fn test_block_addr() {
    let solution_sets = vec![
        SolutionSet {
            solutions: vec![Solution {
                predicate_to_solve: PredicateAddress {
                    contract: ContentAddress([1; 32]),
                    predicate: ContentAddress([1; 32]),
                },
                predicate_data: Default::default(),
                state_mutations: Default::default(),
            }],
        },
        SolutionSet {
            solutions: vec![Solution {
                predicate_to_solve: PredicateAddress {
                    contract: ContentAddress([2; 32]),
                    predicate: ContentAddress([2; 32]),
                },
                predicate_data: Default::default(),
                state_mutations: Default::default(),
            }],
        },
    ];
    let block = Block {
        header: Header {
            number: 0,
            timestamp: Duration::from_secs(0),
        },
        solution_sets: solution_sets.clone(),
    };
    let addr = addr::from_block(&block);
    let content_addr = essential_hash::content_addr(&block);
    assert_eq!(content_addr, addr);

    let set_addrs = solution_sets.iter().rev().map(essential_hash::content_addr);
    let addr = addr::from_header_and_solution_set_addrs(&block.header, set_addrs);
    assert_ne!(content_addr, addr);
}
