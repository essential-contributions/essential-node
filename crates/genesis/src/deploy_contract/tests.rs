use std::{future::Future, pin::Pin, sync::Arc};

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
        directive: Directive::Satisfy,
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
        directive: Directive::Satisfy,
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
        constraints: vec![delta_contract()],
        directive: Directive::Satisfy,
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
        constraints: vec![constrain_keys::constrain_keys()],
        directive: Directive::Satisfy,
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
        directive: Directive::Satisfy,
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

fn make_state_and_solution() -> (
    State<impl Fn(ContentAddress, Key, usize) -> Vec<Vec<Word>>>,
    State<impl Fn(ContentAddress, Key, usize) -> Vec<Vec<Word>>>,
    Arc<Solution>,
) {
    let salt = [0; 32];
    let predicates = [
        Predicate {
            state_read: vec![vec![0]],
            constraints: vec![vec![21]],
            directive: Directive::Satisfy,
        },
        Predicate {
            state_read: vec![vec![5]],
            constraints: vec![vec![21]],
            directive: Directive::Satisfy,
        },
    ];

    let mut predicate_addresses: Vec<_> = predicates
        .iter()
        .map(essential_hash::content_addr)
        .collect();

    let expected_contract_addr = word_4_from_u8_32(
        essential_hash::contract_addr::from_predicate_addrs_slice(&mut predicate_addresses, &salt)
            .0,
    );
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

    let decision_variables = predicates_to_dec_vars(&salt, predicates_to_deploy);

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
