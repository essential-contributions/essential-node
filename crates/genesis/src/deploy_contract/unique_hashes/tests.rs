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

fn hashes_not_unique() -> Access<'static> {
    test_access!({
        let mut access = setup_access();
        access.pre[0] = [vec![0; 4], vec![2; 4], vec![0; 4], vec![12; 4]].concat();
        access
    })
}

#[test]
fn test_unique_hashes() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut stack, mut memory, mut repeat, access) = setup();

    let ops = unique_hashes();
    exec(&mut stack, &mut memory, &mut repeat, &ops, access).unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [1]);

    // Hashes are not unique

    stack.pop().unwrap();
    
    let ops = unique_hashes();
    exec(&mut stack, &mut memory, &mut repeat, &ops, hashes_not_unique()).unwrap();

    let s = stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(s, [0]);
}
