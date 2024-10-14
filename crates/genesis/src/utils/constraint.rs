use essential_constraint_asm::Op;
use essential_constraint_vm::{Access, Memory, OpResult, ProgramControlFlow, Repeat, Stack};

use super::{test_access, TestAccess};

pub fn exec(
    stack: &mut Stack,
    memory: &mut Memory,
    repeat: &mut Repeat,
    ops: &[essential_constraint_asm::Op],
    access: Access<'_>,
) -> OpResult<()> {
    let mut pc = 0;
    while let Some(op) = ops.get(pc) {
        let res = essential_constraint_vm::step_op(access, *op, stack, memory, pc, repeat);
        trace_op_res(pc, &op, &stack, &memory, &repeat, &res);

        let update = match res {
            Ok(update) => update,
            Err(err) => return Err(err),
        };

        match update {
            Some(ProgramControlFlow::Pc(new_pc)) => pc = new_pc,
            Some(ProgramControlFlow::Halt) => break,
            None => pc += 1,
        }
    }
    Ok(())
}

pub fn setup_default() -> (Stack, Memory, Repeat, Access<'static>) {
    let stack = Stack::default();
    let memory = Memory::default();
    let repeat = Repeat::default();
    let access = test_access!(default);
    (stack, memory, repeat, access)
}

pub fn trace_op_res(
    pc: usize,
    op: &Op,
    stack: &Stack,
    memory: &Memory,
    repeat: &Repeat,
    op_res: &OpResult<Option<ProgramControlFlow>>,
) {
    let pc_op = format!("0x{pc:02X}: {op:?}");
    match op_res {
        Ok(_) => {
            tracing::trace!(
                "{pc_op}\n  ├── {:?}\n  |\n  └── {:?}\n  |\n  └── {:?}",
                &stack,
                &memory,
                &repeat
            );
        }
        Err(ref err) => {
            tracing::trace!("{pc_op}");
            tracing::debug!("{err}");
        }
    }
}
