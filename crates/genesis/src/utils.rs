use std::collections::HashMap;

use essential_types::{
    solution::{SolutionData, SolutionDataIndex},
    ContentAddress, Key, PredicateAddress, Value,
};

pub mod constraint;
pub mod state;

#[derive(Debug, Default, Clone)]
pub struct TestAccess {
    pub mut_keys: Vec<Key>,
    pub data: Vec<SolutionData>,
    pub pub_vars: HashMap<SolutionDataIndex, HashMap<Key, Value>>,
    pub index: usize,
    pub pre: Vec<Value>,
    pub post: Vec<Value>,
}

impl TestAccess {
    pub fn with_default_sol_data(mut self) -> Self {
        self.data.push(SolutionData {
            predicate_to_solve: PredicateAddress {
                contract: ContentAddress([0; 32]),
                predicate: ContentAddress([0; 32]),
            },
            decision_variables: Default::default(),
            transient_data: Default::default(),
            state_mutations: Default::default(),
        });
        self
    }
}

macro_rules! test_access {
    (default) => {{
        test_access!(TestAccess::default().with_default_sol_data())
    }};
    ($input:expr) => {{
        use essential_constraint_vm::Access;
        use essential_state_read_vm::{SolutionAccess, StateSlots};
        use essential_types::*;
        use std::collections::HashSet;
        use std::sync::LazyLock;

        static INPUT: LazyLock<TestAccess> = LazyLock::new(|| $input);
        static MUT_KEYS: LazyLock<HashSet<&[Word]>> =
            LazyLock::new(|| INPUT.mut_keys.iter().map(|k| k.as_ref()).collect());
        static SOL_ACC: LazyLock<SolutionAccess> = LazyLock::new(|| SolutionAccess {
            data: INPUT.data.as_ref(),
            index: INPUT.index,
            mutable_keys: &*MUT_KEYS,
            transient_data: &INPUT.pub_vars,
        });
        static STATE_SLOTS: std::sync::LazyLock<StateSlots> = LazyLock::new(|| StateSlots {
            pre: &INPUT.pre,
            post: &INPUT.post,
        });
        static ACCESS: LazyLock<Access> = LazyLock::new(|| essential_state_read_vm::Access {
            solution: *SOL_ACC,
            state_slots: *STATE_SLOTS,
        });
        *ACCESS
    }};
}

pub(crate) use test_access;
