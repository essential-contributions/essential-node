use super::constraint::*;
use super::state_slot_offset;
use super::tags;
use crate::deploy_contract::storage_index;
use essential_constraint_asm as asm;

fn predicate_addrs_i() -> Vec<asm::Op> {
    vec![PUSH(state_slot_offset::PREDICATE_ADDRS), REPC, PUSH(5), MUL]
}

fn predicate_addrs_i_address() -> Vec<asm::Op> {
    [
        predicate_addrs_i(),
        vec![PUSH(1), ADD, PUSH(4), PUSH(0), STATE],
    ]
    .concat()
}

fn predicate_addrs_i_tag() -> Vec<asm::Op> {
    [predicate_addrs_i(), vec![PUSH(1), PUSH(0), STATE]].concat()
}

fn body() -> Vec<asm::Op> {
    [
        vec![PUSH(storage_index::PREDICATES)],
        predicate_addrs_i_address(),
        vec![PUSH(5), PUSH(0), TLD, PUSH(1), ADD, PUSH(0), SWAP, TSTR],
    ]
    .concat()
}

pub fn constrain_keys() -> Vec<u8> {
    let r = [
        vec![PUSH(1), TALC, POP],
        // for i in 0..predicates_size
        read_predicate_size(),
        vec![PUSH(1), REP],
        // match tag
        // NEW_TAG
        match_asm_tag(predicate_addrs_i_tag(), tags::NEW, body()),
        //
        // No match
        panic_on_no_match_asm_tag(predicate_addrs_i_tag()),
        // Loop end
        vec![REPE, PUSH(storage_index::CONTRACTS)],
        read_state_slot(state_slot_offset::CONTRACT_ADDR, 0, 4, false),
        vec![PUSH(5), PUSH(0), TLD, PUSH(1), ADD, PUSH(6), MUL],
        vec![MKEY, EQST],
    ]
    .concat();
    asm::to_bytes(r).collect()
}
