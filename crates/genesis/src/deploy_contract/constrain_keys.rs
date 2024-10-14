use super::constraint::*;
use crate::deploy_contract::dec_var_slot_offset;
use crate::deploy_contract::state_slot_offset;
use essential_constraint_asm as asm;
use essential_types::Word;

#[cfg(test)]
mod tests;

pub fn constrain_keys_bytes() -> Vec<u8> {
    asm::to_bytes(constrain_keys()).collect()
}
/// constraint __mut_keys() == (0..num_predicates)
///     .fold(set, |acc, i| acc.union(set(hashes[i].hash))).union(set(contract_hash));
fn constrain_keys() -> Vec<asm::Op> {
    [
        for_loop([push_hash(), vec![PUSH(4)]].concat()),
        // set's len
        push_num_predicates(),
        vec![
            PUSH(1),
            ADD,
            PUSH(5),
            MUL,
            MKEYS,
            EQST, // compare set
        ],
    ]
    .concat()
}

fn push_hash() -> Vec<asm::Op> {
    vec![
        PUSH(state_slot_offset::GENERATE_HASHES), // slot_ix
        // value_ix
        REPC,
        PUSH(4),
        MUL,
        PUSH(4), // len
        PUSH(0), // delta
        STATE,
    ]
}

fn for_loop(body: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        push_num_predicates(),
        vec![PUSH(1), ADD, PUSH(true as Word), REP],
        body,
        vec![REPE],
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
