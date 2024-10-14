use essential_constraint_vm::{Access, Memory, Repeat, Stack};
use essential_types::{convert::word_from_bytes_slice, Word};

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
        vec![0, 1, 0, 0],
    ];
    access.post = vec![vec![], vec![1, 1, 1, 1]];
    access
}

fn setup() -> (Stack, Memory, Repeat, Access<'static>) {
    let (stack, memory, repeat, _) = setup_default();
    let access = test_access!(setup_access());
    (stack, memory, repeat, access)
}

fn push_word() -> Access<'static> {
    test_access!({
        let mut access = setup_access();

        let mut header = 0;
        header |= 0x0A << 56;
        header |= 0x0B << 48;
        access.data[0].decision_variables[2][2] |= 0x11;
        access.data[0].decision_variables[2][3] = header;
        access
    })
}

#[test]
fn test_check_max() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut stack, mut memory, mut repeat, access) = setup();

    let ops = check_max();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [1]);
}

#[test]
fn test_check_max_state_reads() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut stack, mut memory, mut repeat, access) = setup();

    repeat.repeat_to(0, 3).unwrap();
    stack.push(1).unwrap();
    let ops = check_max_state_reads();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [1]);
}

#[test]
fn test_check_max_state_read_size() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut stack, mut memory, mut repeat, access) = setup();

    repeat.repeat_to(0, 3).unwrap();
    stack.push(1).unwrap();
    let ops = check_max_state_read_size();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [1]);
}

#[test]
fn test_push_dynamic_header_word() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut stack, mut memory, mut repeat, access) = setup();

    repeat.repeat_to(0, 10).unwrap();

    let ops = push_dynamic_header_word();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [0]);
    stack.pop().unwrap();

    for _ in 0..5 {
        repeat.repeat().unwrap();
    }

    let ops = push_dynamic_header_word();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [0]);
    stack.pop().unwrap();

    repeat.repeat().unwrap();

    let ops = push_dynamic_header_word();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [1]);
    stack.pop().unwrap();
}

#[test]
fn test_read_byte() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut stack, mut memory, mut repeat, access) = setup();

    stack
        .push(access.solution.data[0].decision_variables[2][2])
        .unwrap();
    stack.push(7).unwrap();
    let ops = read_byte();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [2]);

    stack.pop().unwrap();

    stack
        .push(access.solution.data[0].decision_variables[2][2])
        .unwrap();
    stack.push(6).unwrap();
    let ops = read_byte();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [3]);
}
