use std::{future::Future, pin::Pin, sync::Arc};

use essential_check::state_read_vm::StateRead;
use essential_types::{
    solution::{Mutation, Solution, SolutionData},
    ContentAddress, Key, PredicateAddress,
};

use super::*;

#[derive(Clone)]
struct State<F>(Arc<F>)
where
    F: Fn(ContentAddress, Key, usize) -> Vec<Vec<Word>>;

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
    let predicate_to_solve = PredicateAddress {
        contract: essential_hash::content_addr(&contract),
        predicate: essential_hash::content_addr(&contract.predicates[0]),
    };

    let predicate = Arc::new(contract.predicates[0].clone());
    let salt = [0; 32];

    let p = [
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

    let mut pa: Vec<_> = p.iter().map(essential_hash::content_addr).collect();

    let expect = word_4_from_u8_32(
        essential_hash::contract_addr::from_predicate_addrs_slice(&mut pa, &salt).0,
    );
    dbg!(expect);
    let p0a = word_4_from_u8_32(pa[0].0);
    let p1a = word_4_from_u8_32(pa[1].0);
    dbg!(p0a);
    dbg!(p1a);

    let p0 = essential_hash::content_addr(&p[0]);
    let predicates_to_deploy = [
        DeployedPredicate::New(&p[1]),
        DeployedPredicate::Existing(&p0),
    ];

    let decision_variables = predicates_to_dec_vars(&salt, predicates_to_deploy);

    let contract_mutation = Mutation {
        key: vec![0, expect[0], expect[1], expect[2], expect[3]],
        value: vec![1],
    };
    let p0_mutation = Mutation {
        key: vec![1, p0a[0], p0a[1], p0a[2], p0a[3]],
        value: vec![1],
    };
    let p1_mutation = Mutation {
        key: vec![1, p1a[0], p1a[1], p1a[2], p1a[3]],
        value: vec![1],
    };

    let data = SolutionData {
        predicate_to_solve,
        decision_variables,
        transient_data: Default::default(),
        state_mutations: vec![contract_mutation.clone()],
    };
    let solution = Solution { data: vec![data] };
    let solution = Arc::new(solution);

    let p0_mut = p0_mutation.clone();
    let pre = move |_, key: Key, _| {
        if key == p0_mut.key {
            vec![p0_mut.value.clone()]
        } else {
            vec![]
        }
    };
    let post = move |_, key: Key, _| {
        if key == contract_mutation.key {
            vec![contract_mutation.value.clone()]
        } else if key == p0_mutation.key {
            vec![p0_mutation.value.clone()]
        } else if key == p1_mutation.key {
            vec![p1_mutation.value.clone()]
        } else {
            vec![]
        }
    };
    let pre = State(Arc::new(pre));
    let post = State(Arc::new(post));

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
