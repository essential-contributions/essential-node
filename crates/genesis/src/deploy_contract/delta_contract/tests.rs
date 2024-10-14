use essential_constraint_vm::{Access, Memory, Repeat, Stack};
use essential_types::{convert::word_from_bytes_slice, ContentAddress, Word};

use crate::utils::{
    constraint::{exec, setup_default},
    test_access, TestAccess,
};

use super::*;

fn setup_access() -> TestAccess {
    let mut access = TestAccess::default().with_default_sol_data();
    let mut header: Word = 0;
    // num state reads
    header |= 0x02 << 56;
    // num constraints
    header |= 0x03 << 48;
    // state read 0 len
    header |= 0x01 << 40;
    // state read 1 len
    header |= 0x02 << 32;
    // constraint 0 len
    header |= 0x01 << 24;
    // constraint 1 len
    header |= 0x02 << 16;
    // constraint 2 len
    header |= 0x03 << 8;
    let mut encoded = vec![];
    let programs = [0u8; 9];
    header |= programs[0] as Word;
    encoded.push(word_from_bytes_slice(&programs[1..9]));

    access.data[0].decision_variables = vec![
        vec![3],
        vec![1, 2, 3, 4],
        [vec![0, 16, header], encoded.clone()].concat(),
        vec![1, 32, 1, 2, 3, 4],
        [vec![0, 16, header], encoded].concat(),
    ];

    access.pre = vec![
        [vec![0; 4], vec![1; 4], vec![2; 4], vec![3; 4]].concat(),
        vec![1, 1, 1, 0],
    ];
    access.post = vec![vec![], vec![1, 1, 1, 1]];
    access
}

fn setup() -> (Stack, Memory, Repeat, Access<'static>) {
    let (stack, memory, repeat, _) = setup_default();
    let access = test_access!(setup_access());
    (stack, memory, repeat, access)
}

fn neg_num_predicates() -> Access<'static> {
    test_access!({
        let mut access = setup_access();
        access.data[0].decision_variables[0][0] = -1;
        access
    })
}

fn zero_num_predicates() -> Access<'static> {
    test_access!({
        let mut access = setup_access();
        access.data[0].decision_variables[0][0] = 0;
        access
    })
}

fn pre_contract_exists() -> Access<'static> {
    test_access!({
        let mut access = setup_access();
        access.pre[1][3] = 1;
        access
    })
}

fn post_contract_does_not_exist() -> Access<'static> {
    test_access!({
        let mut access = setup_access();
        access.post[1][3] = 0;
        access
    })
}

#[test]
fn test_delta_contract() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut stack, mut memory, mut repeat, access) = setup();

    let ops = delta_contract();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [1]);

    stack.pop().unwrap();

    let ops = delta_contract();
    exec(
        &mut stack,
        &mut memory,
        &mut repeat,
        &ops,
        neg_num_predicates(),
    )
    .unwrap_err();

    let mut stack = Stack::default();

    let ops = delta_contract();
    exec(
        &mut stack,
        &mut memory,
        &mut repeat,
        &ops,
        zero_num_predicates(),
    )
    .unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [0]);

    stack.pop().unwrap();

    let ops = delta_contract();
    exec(
        &mut stack,
        &mut memory,
        &mut repeat,
        &ops,
        pre_contract_exists(),
    )
    .unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [0]);

    stack.pop().unwrap();

    let ops = delta_contract();
    exec(
        &mut stack,
        &mut memory,
        &mut repeat,
        &ops,
        post_contract_does_not_exist(),
    )
    .unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [0]);
}
