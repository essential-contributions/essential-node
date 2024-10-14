use crate::deploy_contract::dec_var_slot_offset;

use super::constraint::*;
use super::state_slot_offset;
use essential_constraint_asm as asm;
use essential_types::Word;

#[cfg(test)]
mod tests;

pub fn delta_contract_bytes() -> Vec<u8> {
    asm::to_bytes(delta_contract()).collect()
}

enum Delta {
    Pre = 0,
    Post = 1,
}

/// constraint num_predicates > 0 && !found_contract && found_contract';
fn delta_contract() -> Vec<asm::Op> {
    [
        push_num_predicates(),
        vec![DUP, PUSH(0), GT, SWAP, DUP], // [num_predicates > 0, num_predicates, num_predicates]
        push_found_contract(Delta::Pre),   // [num_predicates > 0, num_predicates, found_contract]
        vec![NOT, SWAP],                   // [num_predicates > 0, !found_contract, num_predicates]
        push_found_contract(Delta::Post),  // [num_predicates > 0, !found_contract, found_contract']
        vec![AND, AND],
    ]
    .concat()
}

/// # Args
/// number of predicates
fn push_found_contract(delta: Delta) -> Vec<asm::Op> {
    vec![
        PUSH(state_slot_offset::FIND_CONTRACT), // slot_ix
        SWAP,                                   // value_ix
        PUSH(1),                                // len
        PUSH(delta as Word),                    // delta
        STATE,
    ]
}

fn push_num_predicates() -> Vec<asm::Op> {
    vec![
        PUSH(dec_var_slot_offset::NUM_PREDICATES), // slot_ix
        PUSH(0),                                   // value_ix
        PUSH(1),                                   // len
        VAR,
    ]
}
