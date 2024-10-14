use essential_constraint_vm::Access;
use essential_state_read_vm::Vm;
use essential_types::convert::{bytes_from_word, word_4_from_u8_32, word_from_bytes_slice};

use crate::utils::{
    state::{exec, setup_default, TestState},
    test_access, TestAccess,
};

use super::*;

fn setup() -> (Vm, TestState, Access<'static>) {
    let (mut vm, state, _) = setup_default();
    vm.repeat.repeat_to(0, 2).unwrap();
    let access = test_access!({
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
        access
    });
    (vm, state, access)
}

#[tokio::test]
async fn test_body() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut vm, state, access) = setup();
    vm.repeat.repeat_to(0, 3).unwrap();
    vm.state_memory.alloc_slots(1).unwrap();

    let ops = hash_new_body();
    exec(&mut vm, &ops, access, &state).await.unwrap();

    let expected = access.solution.data[0].decision_variables[2][2..].to_vec();
    let expected: Vec<_> = expected
        .into_iter()
        .flat_map(bytes_from_word)
        .take(16)
        .collect();
    let expected_stack = word_4_from_u8_32(essential_hash::hash_bytes(&expected));
    let stack = vm.stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(stack, expected_stack);
    let expected_mem = [vec![0], expected_stack.to_vec()].concat();
    assert_eq!(vm.state_memory[0], expected_mem);

    vm.repeat.repeat().unwrap();
    vm.pc = 0;

    let ops = existing_body();
    exec(&mut vm, &ops, access, &state).await.unwrap();

    let stack = vm.stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(stack, [expected_stack.to_vec(), vec![1, 2, 3, 4]].concat());
    assert_eq!(vm.state_memory[0][5..], [1, 1, 2, 3, 4]);

    vm.repeat.repeat().unwrap();
    vm.pc = 0;

    let ops = hash_new_body();
    exec(&mut vm, &ops, access, &state).await.unwrap();

    let stack = vm.stack.iter().copied().collect::<Vec<_>>();
    assert_eq!(
        stack,
        [
            expected_stack.to_vec(),
            vec![1, 2, 3, 4],
            expected_stack.to_vec()
        ]
        .concat()
    );
    assert_eq!(vm.state_memory[0][10..], expected_mem);
}

fn generate_stack(access: Access<'_>) -> Vec<Word> {
    let stack = access.solution.data[0].decision_variables[2][2..].to_vec();
    let stack: Vec<_> = stack
        .into_iter()
        .flat_map(bytes_from_word)
        .take(16)
        .collect();
    let stack = word_4_from_u8_32(essential_hash::hash_bytes(&stack));
    let stack = [stack, [1, 2, 3, 4], stack].concat();

    stack
}

fn generate_mem(stack: &[Word]) -> Vec<Word> {
    [
        vec![0],
        stack[0..4].to_vec(),
        vec![1],
        stack[4..8].to_vec(),
        vec![0],
        stack[8..12].to_vec(),
    ]
    .concat()
}

fn generate_contract_hash(stack: &[Word]) -> [Word; 4] {
    let contract_bytes: Vec<_> = stack
        .iter()
        .copied()
        .chain([1, 2, 3, 4])
        .flat_map(bytes_from_word)
        .collect();
    word_4_from_u8_32(essential_hash::hash_bytes(&contract_bytes))
}

#[tokio::test]
async fn test_hash_contract() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut vm, state, access) = setup();
    vm.state_memory.alloc_slots(1).unwrap();

    let stack = generate_stack(access);

    // Contract storage location
    vm.stack.push(0).unwrap(); // slot_ix
    vm.stack.push(3 * SIZE_OF_TAG_HASH).unwrap(); // value_ix

    for word in &stack {
        vm.stack.push(*word).unwrap();
    }

    let mut mem = generate_mem(&stack);

    vm.state_memory.store(0, 0, mem.clone()).unwrap();

    let ops = hash_contract();
    exec(&mut vm, &ops, access, &state).await.unwrap();
    assert!(vm.stack.is_empty(), "{:?}", vm.stack);

    let contract_hash = generate_contract_hash(&stack);
    mem.extend(contract_hash);
    assert_eq!(vm.state_memory[0], mem);
}

#[tokio::test]
async fn test_generate_hashes() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut vm, state, access) = setup();

    let ops = generate_hashes();
    exec(&mut vm, &ops, access, &state).await.unwrap();

    assert!(vm.stack.is_empty(), "{:?}", vm.stack);

    let stack = generate_stack(access);
    let mut mem = generate_mem(&stack);
    mem.extend(generate_contract_hash(&stack));
    assert_eq!(vm.state_memory[0], mem);
}
