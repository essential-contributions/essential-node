use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use essential_check::state_read_vm::StateRead;
use essential_types::{
    solution::{Solution, SolutionData},
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
    let pre = State(Arc::new(|_, _, _| vec![]));
    let post = State(Arc::new(|_, _, _| vec![]));

    let contract = create();
    let predicate_to_solve = PredicateAddress {
        contract: essential_hash::content_addr(&contract),
        predicate: essential_hash::content_addr(&contract.predicates[0]),
    };

    let predicate = Arc::new(contract.predicates[0].clone());
    let salt = [0; 32];

    let p: HashMap<_, _> = [
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
    ]
    .into_iter()
    .map(|p| (essential_hash::content_addr(&p), p))
    .collect();

    let mut pa: Vec<_> = p.keys().cloned().collect();

    dbg!(word_4_from_u8_32(
        essential_hash::contract_addr::from_predicate_addrs_slice(&mut pa, &salt).0
    ));

    let p1 = essential_hash::content_addr(&p[&pa[1]]);
    let predicates_to_deploy = [
        DeployedPredicate::New(&p[&pa[0]]),
        DeployedPredicate::Existing(&p1),
    ];

    let decision_variables = predicates_to_dec_vars(&salt, predicates_to_deploy);

    let data = SolutionData {
        predicate_to_solve,
        decision_variables,
        transient_data: Default::default(),
        state_mutations: vec![],
    };
    let solution = Solution { data: vec![data] };
    let solution = Arc::new(solution);

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
