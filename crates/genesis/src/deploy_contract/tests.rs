use std::{future::Future, i64, pin::Pin, sync::Arc};

use essential_check::state_read_vm::StateRead;
use essential_types::{
    solution::{Mutation, Solution, SolutionData},
    ContentAddress, Key, PredicateAddress,
};
use read_contract_addr::read_contract_addr;
use read_predicate_addr::read_predicate_addr;

use super::*;

struct State<F>(Arc<F>)
where
    F: Fn(ContentAddress, Key, usize) -> Vec<Vec<Word>>;

impl<F> Clone for State<F>
where
    F: Fn(ContentAddress, Key, usize) -> Vec<Vec<Word>>,
{
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<F> StateRead for State<F>
where
    F: Fn(ContentAddress, Key, usize) -> Vec<Vec<Word>>,
{
    type Error = String;

    type Future = Pin<Box<dyn Future<Output = Result<Vec<Vec<Word>>, Self::Error>> + Send>>;

    fn key_range(
        &self,
        contract_addr: ContentAddress,
        key: Key,
        num_values: usize,
    ) -> Self::Future {
        let r = self.0(contract_addr, key, num_values);
        Box::pin(async move { Ok(r) })
    }
}

#[tokio::test]
async fn test_deploy() {
    let _ = tracing_subscriber::fmt::try_init();
    let contract = create();

    let predicate = Arc::new(contract.predicates[0].clone());
    let (pre, post, solution) = make_state_and_solution();

    essential_check::solution::check_predicates(
        &pre,
        &post,
        solution,
        |_| predicate.clone(),
        Default::default(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_read_contract_addr() {
    let _ = tracing_subscriber::fmt::try_init();
    let state_read = read_contract_addr();

    let predicate = Predicate {
        state_read: vec![state_read],
        constraints: vec![],
    };

    let predicate = Arc::new(predicate);

    let (pre, post, solution) = make_state_and_solution();

    essential_check::solution::check_predicates(
        &pre,
        &post,
        solution,
        |_| predicate.clone(),
        Default::default(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_read_predicate_addr() {
    let _ = tracing_subscriber::fmt::try_init();

    let predicate = Predicate {
        state_read: vec![read_contract_addr(), read_predicate_addr()],
        constraints: vec![],
    };

    let predicate = Arc::new(predicate);

    let (pre, post, solution) = make_state_and_solution();

    essential_check::solution::check_predicates(
        &pre,
        &post,
        solution,
        |_| predicate.clone(),
        Default::default(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_delta_contract() {
    let _ = tracing_subscriber::fmt::try_init();

    let predicate = Predicate {
        state_read: vec![read_contract_addr(), read_predicate_addr()],
        constraints: vec![delta_contract::delta_contract_bytes()],
    };

    let predicate = Arc::new(predicate);

    let (pre, post, solution) = make_state_and_solution();

    essential_check::solution::check_predicates(
        &pre,
        &post,
        solution,
        |_| predicate.clone(),
        Default::default(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_constrain_keys() {
    let _ = tracing_subscriber::fmt::try_init();

    let predicate = Predicate {
        state_read: vec![read_contract_addr(), read_predicate_addr()],
        constraints: vec![constrain_keys::constrain_keys_bytes()],
    };

    let predicate = Arc::new(predicate);

    let (pre, post, solution) = make_state_and_solution();

    essential_check::solution::check_predicates(
        &pre,
        &post,
        solution,
        |_| predicate.clone(),
        Default::default(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_check_exists() {
    let _ = tracing_subscriber::fmt::try_init();

    let predicate = Predicate {
        state_read: vec![read_contract_addr(), read_predicate_addr()],
        constraints: vec![check_exists::check_exists()],
    };

    let predicate = Arc::new(predicate);

    let (pre, post, solution) = make_state_and_solution();

    essential_check::solution::check_predicates(
        &pre,
        &post,
        solution,
        |_| predicate.clone(),
        Default::default(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_validate_contract() {
    let _ = tracing_subscriber::fmt::try_init();

    let predicate = Predicate {
        state_read: vec![read_contract_addr(), read_predicate_addr()],
        constraints: vec![validate_contract::validate_contract()],
    };

    let predicate = Arc::new(predicate);

    let (pre, post, solution) = make_state_and_solution();

    essential_check::solution::check_predicates(
        &pre,
        &post,
        solution,
        |_| predicate.clone(),
        Default::default(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_inception() {
    let _ = tracing_subscriber::fmt::try_init();
    let contract = create();

    let predicate = Arc::new(contract.predicates[0].clone());
    let (pre, post, solution) = make_inception_state_and_solution(&contract);

    essential_check::solution::check_predicates(
        &pre,
        &post,
        solution,
        |_| predicate.clone(),
        Default::default(),
    )
    .await
    .unwrap();
}

fn make_state_and_solution() -> (
    State<impl Fn(ContentAddress, Key, usize) -> Vec<Vec<Word>>>,
    State<impl Fn(ContentAddress, Key, usize) -> Vec<Vec<Word>>>,
    Arc<Solution>,
) {
    let salt = [0; 32];
    let predicates = [
        Predicate {
            state_read: vec![vec![0, 4], vec![12]],
            constraints: vec![vec![21, 99, 88]],
        },
        Predicate {
            state_read: vec![vec![5, 90, 55, 1], vec![99, 99, 99]],
            constraints: vec![
                vec![21, 4, 5, 6, 7],
                vec![21, 4, 5, 6, 7],
                vec![21, 4, 5, 6, 7],
            ],
        },
    ];

    let mut predicate_addresses: Vec<_> = predicates
        .iter()
        .map(essential_hash::content_addr)
        .collect();

    let ca =
        essential_hash::contract_addr::from_predicate_addrs_slice(&mut predicate_addresses, &salt);
    let expected_contract_addr = word_4_from_u8_32(ca.0);
    let predicate_addr_words = predicate_addresses
        .iter()
        .map(|a| word_4_from_u8_32(a.0))
        .collect::<Vec<_>>();

    tracing::debug!("Expected contract words {:?}", expected_contract_addr);
    tracing::debug!("Expected predicate words {:?}", predicate_addr_words);

    let predicates_to_deploy = [
        DeployedPredicate::Existing(&predicate_addresses[0]),
        DeployedPredicate::New(&predicates[1]),
    ];

    let decision_variables = predicates_to_dec_vars(&salt, predicates_to_deploy).unwrap();
    // for (i, v) in decision_variables.iter().enumerate() {
    //     println!("Decision variable slot {}:", i);
    //     for (j, w) in v.iter().enumerate() {
    //         println!("  Word {}: {:016X}", j, w);
    //         // for (k, b) in w.to_be_bytes().iter().enumerate() {
    //         //     println!("    Byte {}: {:02X}", k, b);
    //         // }
    //     }
    // }
    // panic!("{:?}", &decision_variables);

    let contract_mutation = Mutation {
        key: vec![
            0,
            expected_contract_addr[0],
            expected_contract_addr[1],
            expected_contract_addr[2],
            expected_contract_addr[3],
        ],
        value: vec![1],
    };

    let mut mutations = predicate_addr_words
        .iter()
        .map(|a| Mutation {
            key: vec![1, a[0], a[1], a[2], a[3]],
            value: vec![1],
        })
        .collect::<Vec<_>>();

    mutations.push(contract_mutation);

    let data = SolutionData {
        predicate_to_solve: PredicateAddress {
            contract: ContentAddress([0; 32]),
            predicate: ContentAddress([0; 32]),
        },
        decision_variables,
        transient_data: Default::default(),
        state_mutations: mutations
            .iter()
            .enumerate()
            .filter_map(|(i, m)| if i == 0 { None } else { Some(m.clone()) })
            .collect(),
    };
    let solution = Solution { data: vec![data] };
    let solution = Arc::new(solution);

    let p0_mut = mutations[0].clone();
    let pre = move |_, key: Key, _| {
        if key == p0_mut.key {
            vec![p0_mut.value.clone()]
        } else {
            vec![]
        }
    };
    let post = move |_, key: Key, _| {
        mutations
            .iter()
            .find_map(|m| {
                if key == m.key {
                    Some(vec![m.value.clone()])
                } else {
                    None
                }
            })
            .unwrap_or(vec![])
    };
    let pre = State(Arc::new(pre));
    let post = State(Arc::new(post));

    (pre, post, solution)
}

fn make_inception_state_and_solution(
    contract: &Contract,
) -> (
    State<impl Fn(ContentAddress, Key, usize) -> Vec<Vec<Word>>>,
    State<impl Fn(ContentAddress, Key, usize) -> Vec<Vec<Word>>>,
    Arc<Solution>,
) {
    let predicates_to_deploy = contract.predicates.iter().map(DeployedPredicate::New);

    let predicate_addr_words = contract
        .predicates
        .iter()
        .map(|p| word_4_from_u8_32(essential_hash::content_addr(p).0))
        .collect::<Vec<_>>();
    let contract_addr = essential_hash::contract_addr::from_predicate_addrs(
        contract.predicates.iter().map(essential_hash::content_addr),
        &contract.salt,
    );
    let contract_addr_words = word_4_from_u8_32(contract_addr.0);

    let decision_variables = predicates_to_dec_vars(&contract.salt, predicates_to_deploy).unwrap();

    let contract_mutation = Mutation {
        key: vec![
            0,
            contract_addr_words[0],
            contract_addr_words[1],
            contract_addr_words[2],
            contract_addr_words[3],
        ],
        value: vec![1],
    };

    let mut mutations = predicate_addr_words
        .iter()
        .map(|a| Mutation {
            key: vec![1, a[0], a[1], a[2], a[3]],
            value: vec![1],
        })
        .collect::<Vec<_>>();

    mutations.push(contract_mutation);

    let data = SolutionData {
        predicate_to_solve: PredicateAddress {
            contract: ContentAddress([0; 32]),
            predicate: ContentAddress([0; 32]),
        },
        decision_variables,
        transient_data: Default::default(),
        state_mutations: mutations.clone(),
    };
    let solution = Solution { data: vec![data] };
    let solution = Arc::new(solution);

    let pre = move |_, _, _| vec![];
    let post = move |_, key: Key, _| {
        mutations
            .iter()
            .find_map(|m| {
                if key == m.key {
                    Some(vec![m.value.clone()])
                } else {
                    None
                }
            })
            .unwrap_or(vec![])
    };
    let pre = State(Arc::new(pre));
    let post = State(Arc::new(post));

    (pre, post, solution)
}

#[test]
fn feature() {
    // let i: u8 = 2;
    // let w = [i, 3, 99, 7, 9, 22, 8, 1];
    // let word = word_from_bytes(w);
    // let n = word >> (7 * 8);
    // assert_eq!(n, 2);

    // let i: u8 = 2;
    // let w = [3, i, 99, 7, 9, 22, 8, 1];
    // let word = word_from_bytes(w);
    // println!("{:016X}", word);
    // let word = word & 0x00FF000000000000;
    // println!("{:016X}", word);
    // let n = word >> (6i64 * 8);
    // // let n = n & 0x000000000000FF;
    // assert_eq!(n, 2);
    // let i: i64 = 72057594037927936;
    // println!("{:016X}", i);
    // println!("{:016X}", i as u64);

    let word: i64 = 0x000100020004FFFF;

    for i in 0..8 {
        let byte = read_byte(i, word);
        println!("{:016X}", byte);
    }
    println!();

    let bytes = read_bytes(0..2, word);
    println!("{:016X}", bytes);

    let bytes = read_bytes(0..7, word);
    println!("{:016X}", bytes);

    let bytes = read_bytes(5..7, word);
    println!("{:016X}", bytes);

    println!();

    for i in 0..20 {
        // Derive two byte range from i
        let bytes = read_u16(i, word);
        let word_ix = calc_word(i);
        println!("{i}: word {word_ix}: {:016X}: {}", bytes, bytes);
    }
}

fn calc_word(index: u32) -> i64 {
    ((index + 1) / 4) as i64
}

fn read_u16(i: u32, word: i64) -> i64 {
    let s = ((i + 1) % 4) * 2 + 1;
    let e = ((i + 1) % 4) * 2;
    // dbg!(s, e);
    let start = 7 - s;
    let end = 7 - e;
    // dbg!(start, end);
    let byte_0 = read_byte(start, word);
    let byte_1 = read_byte(end, word);
    byte_0 | (byte_1 << 8)
}

fn read_byte(index: u32, word: i64) -> i64 {
    let mask: i64 = 0xFF << (index * 8);
    let byte = word & mask;
    println!(
        "word: {:016X}, mask: {:016X}, byte: {:016X}",
        word, mask, byte
    );
    (byte as u64 >> (index * 8)) as i64
}

fn read_bytes(range: std::ops::Range<u32>, word: i64) -> i64 {
    assert!(range.end < 8);
    let mut mask: i64 = 0;
    for i in range.clone() {
        mask |= 0xFF << (i * 8);
    }
    let byte = word & mask;
    (byte as u64 >> (range.start * 8)) as i64
}
