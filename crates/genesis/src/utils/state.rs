use std::{collections::HashMap, convert::Infallible, future::Future, pin::Pin};

use essential_state_asm::Op;
use essential_state_read_vm::{asm, error::StateReadError, Access, GasLimit, StateRead, Vm};
use essential_types::{ContentAddress, Key, Value};

use super::{test_access, TestAccess};

#[derive(Default)]
pub struct TestState(pub HashMap<ContentAddress, HashMap<Key, Value>>);

impl StateRead for TestState {
    type Error = Infallible;

    type Future = Pin<Box<dyn Future<Output = Result<Vec<Value>, Self::Error>>>>;

    fn key_range(
        &self,
        contract_addr: ContentAddress,
        key: Key,
        _num_values: usize,
    ) -> Self::Future {
        let v = match self.0.get(&contract_addr) {
            Some(m) => m.get(&key).cloned().unwrap_or_default(),
            None => vec![],
        };
        Box::pin(async move { Ok(vec![v]) })
    }
}

pub async fn exec(
    vm: &mut Vm,
    ops: &[asm::Op],
    access: Access<'_>,
    state: &TestState,
) -> Result<(), StateReadError<Infallible>> {
    vm.exec_ops(ops, access, state, &|_: &Op| 1, GasLimit::UNLIMITED)
        .await
        .map(|_| ())
}

pub fn setup_default() -> (Vm, TestState, Access<'static>) {
    let vm = Vm::default();
    let state = TestState::default();
    let access = test_access!(default);
    (vm, state, access)
}
