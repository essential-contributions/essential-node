use essential_types::convert::{bytes_from_word, word_4_from_u8_32};

use crate::utils::{
    state::{exec, setup_default},
    test_access, TestAccess,
};

use super::*;

#[tokio::test]
async fn test_load_last_addr() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut vm, state, access) = setup_default();
    vm.repeat.repeat_to(0, 50).unwrap();
    vm.state_memory
        .alloc_slots(state_mem_offset::PREDICATE_ADDRS as usize + 1)
        .unwrap();
    vm.state_memory
        .store(
            state_mem_offset::PREDICATE_ADDRS as usize,
            0,
            vec![1, 2, 3, 4, 5],
        )
        .unwrap();
    let ops = load_last_addr();
    exec(&mut vm, &ops, access, &state).await.unwrap();
    let stack = vm.stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(stack, vec![2, 3, 4, 5]);
}

#[tokio::test]
async fn test_new_tag_body() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut vm, state, _) = setup_default();

    let access = test_access!({
        let mut access = TestAccess::default().with_default_sol_data();
        access.data[0].decision_variables = vec![vec![0], vec![0], vec![0], vec![0, 24, 2, 3, 4]];
        access
    });
    vm.repeat.repeat_to(0, 50).unwrap();
    vm.repeat.repeat().unwrap();
    let ops = new_tag_body();
    exec(&mut vm, &ops, access, &state).await.unwrap();
    let expected = word_4_from_u8_32(essential_hash::hash_bytes(
        (2..5)
            .flat_map(bytes_from_word)
            .collect::<Vec<_>>()
            .as_slice(),
    ));
    assert_eq!(vm.stack.iter().copied().collect::<Vec<_>>(), expected);
}

#[tokio::test]
async fn test_body_setup() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut vm, state, _) = setup_default();

    let access = test_access!({
        let mut access = TestAccess::default().with_default_sol_data();
        access.data[0].decision_variables = vec![vec![0], vec![0], vec![0], vec![0, 24, 2, 3, 4]];
        access
    });
    vm.repeat.repeat_to(0, 50).unwrap();
    vm.repeat.repeat().unwrap();
    let ops = body_setup();
    exec(&mut vm, &ops, access, &state).await.unwrap();
    let stack = vm.stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(stack, vec![1, 3, 4, 5]);
}
