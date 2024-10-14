use essential_constraint_vm::Access;
use essential_state_read_vm::{StateMemory, Vm};
use essential_types::{convert::word_from_bytes_slice, ContentAddress};

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

        access.pre = vec![[vec![0; 4], vec![1; 4], vec![2; 4], vec![3; 4]].concat()];
        access
    });
    (vm, state, access)
}

#[tokio::test]
async fn test_find_contract() {
    let _ = tracing_subscriber::fmt::try_init();
    let (mut vm, mut state, access) = setup();

    let ops = find_contract();
    exec(&mut vm, &ops, access, &state).await.unwrap();

    assert!(vm.stack.is_empty(), "{:?}", vm.stack);
    assert_eq!(vm.state_memory[0], [0, 0, 0, 0]);

    // Existing predicate exists
    let contract_addr = ContentAddress([0; 32]);
    state
        .0
        .entry(contract_addr.clone())
        .or_default()
        .insert(vec![1; 5], vec![1]);

    vm.pc = 0;
    vm.state_memory = StateMemory::default();

    let ops = find_contract();
    exec(&mut vm, &ops, access, &state).await.unwrap();

    assert!(vm.stack.is_empty(), "{:?}", vm.stack);
    assert_eq!(vm.state_memory[0], [0, 1, 0, 0]);

    // Contract exists
    state
        .0
        .entry(contract_addr.clone())
        .or_default()
        .insert([vec![0], vec![3; 4]].concat(), vec![1]);

    vm.pc = 0;
    vm.state_memory = StateMemory::default();

    let ops = find_contract();
    exec(&mut vm, &ops, access, &state).await.unwrap();

    assert!(vm.stack.is_empty(), "{:?}", vm.stack);
    assert_eq!(vm.state_memory[0], [0, 1, 0, 1]);
    
    // All existing
    state
        .0
        .entry(contract_addr.clone())
        .or_default()
        .insert([vec![1], vec![0; 4]].concat(), vec![1]);

    state
        .0
        .entry(contract_addr.clone())
        .or_default()
        .insert([vec![1], vec![2; 4]].concat(), vec![1]);

    vm.pc = 0;
    vm.state_memory = StateMemory::default();

    let ops = find_contract();
    exec(&mut vm, &ops, access, &state).await.unwrap();

    assert!(vm.stack.is_empty(), "{:?}", vm.stack);
    assert_eq!(vm.state_memory[0], [1, 1, 1, 1]);
    
    // Invalid values
    state
        .0
        .entry(contract_addr.clone())
        .or_default()
        .insert([vec![0], vec![3; 4]].concat(), vec![3]);

    state
        .0
        .entry(contract_addr.clone())
        .or_default()
        .insert(vec![1; 5], vec![0]);
    
    state
        .0
        .entry(contract_addr.clone())
        .or_default()
        .insert([vec![1], vec![0; 4]].concat(), vec![-100]);

    state
        .0
        .entry(contract_addr.clone())
        .or_default()
        .insert([vec![1], vec![2; 4]].concat(), vec![3000]);

    vm.pc = 0;
    vm.state_memory = StateMemory::default();

    let ops = find_contract();
    exec(&mut vm, &ops, access, &state).await.unwrap();

    assert!(vm.stack.is_empty(), "{:?}", vm.stack);
    assert_eq!(vm.state_memory[0], [0, 0, 0, 0]);

}
