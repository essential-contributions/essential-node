use essential_types::ContentAddress;

use crate::utils::state::{exec, setup_default};

use super::*;

#[tokio::test]
async fn test_alloc() {
    let ops = alloc(10);
    let (mut vm, state, access) = setup_default();
    exec(&mut vm, &ops, access, &state).await.unwrap();
    assert_eq!(vm.state_memory.len(), 10);
}

#[tokio::test]
async fn test_read_single_key() {
    let ops = [alloc(3), read_single_key(5, 2)].concat();
    let (mut vm, mut state, access) = setup_default();

    let key = vec![1, 2, 3, 4, 5];
    let value = vec![6, 7, 8, 9, 10];
    let storage = [(key.clone(), value.clone())].into_iter().collect();
    state.0.insert(ContentAddress([0; 32]), storage);

    for k in &key {
        vm.stack.push(*k).unwrap();
    }

    exec(&mut vm, &ops, access, &state).await.unwrap();

    assert_eq!(vm.state_memory[2], value);
}
