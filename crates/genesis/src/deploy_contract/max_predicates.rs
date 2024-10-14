use super::constraint::*;
use crate::deploy_contract::dec_var_slot_offset;
use essential_constraint_asm as asm;
use essential_types::contract::Contract;
use essential_types::Word;

#[cfg(test)]
mod tests;

pub fn max_predicates_bytes() -> Vec<u8> {
    asm::to_bytes(max_predicates()).collect()
}
/// constraint num_predicates <= MAX_PREDICATES && __num_slots(Slots::Var) == num_predicates + 2;
fn max_predicates() -> Vec<asm::Op> {
    [
        push_num_predicates(),
        vec![
            DUP,
            PUSH(Contract::MAX_PREDICATES as Word),
            LTE,
            SWAP,
            PUSH(2),
            ADD,
            PUSH(0),
            NSLT,
            EQ,
            AND,
        ],
    ]
    .concat()
}

fn push_num_predicates() -> Vec<asm::Op> {
    vec![
        PUSH(dec_var_slot_offset::NUM_PREDICATES), // slot_ix
        PUSH(0),                                   // value_ix
        PUSH(1),                                   // len
        VAR,
    ]
}
