use super::constraint::*;
use super::state_slot_offset;
use super::tags;
use crate::{deploy_contract::storage_index, utils::constraint::*};
use essential_constraint_asm as asm;

fn predicate_addrs_i() -> Vec<asm::Op> {
    ops![
        PUSH: state_slot_offset::PREDICATE_ADDRS,
        REPEAT_COUNTER,
        PUSH: 5,
        MUL,
    ]
}

fn predicate_addrs_i_address() -> Vec<asm::Op> {
    [
        predicate_addrs_i(),
        ops![
            PUSH: 1,
            ADD,
            PUSH: 4,
            PUSH: 0,
            STATE,
        ],
    ]
    .concat()
}

fn predicate_addrs_i_tag() -> Vec<asm::Op> {
    [
        predicate_addrs_i(),
        ops![
            PUSH: 1,
            PUSH: 0,
            STATE,
        ],
    ]
    .concat()
}

fn body() -> Vec<asm::Op> {
    [
        ops![PUSH: storage_index::PREDICATES],
        predicate_addrs_i_address(),
        ops![
            PUSH: 5,
            PUSH: 0,
            TEMP_LOAD,
            PUSH: 1,
            ADD,
            PUSH: 0,
            SWAP,
            TEMP_STORE,
        ],
    ]
    .concat()
}

pub fn constrain_keys() -> Vec<u8> {
    let r = [
        ops![
            PUSH: 1,
            TEMP_ALLOC,
            POP,
        ],
        // for i in 0..predicates_size
        read_predicate_size(),
        ops![
            PUSH: 1,
            REPEAT
        ],
        // match tag
        // NEW_TAG
        match_asm_tag(predicate_addrs_i_tag(), tags::NEW, body()),
        //
        // No match
        panic_on_no_match_asm_tag(predicate_addrs_i_tag()),
        // Loop end
        ops![
            REPEAT_END,
            PUSH: storage_index::CONTRACTS,
        ],
        read_state_slot(state_slot_offset::CONTRACT_ADDR, 0, 4, false),
        ops![
            PUSH: 5,
            PUSH: 0,
            TEMP_LOAD,
            PUSH: 1,
            ADD,
            PUSH: 6,
            MUL,
        ],
        ops![MUT_KEYS, EQ_SET],
    ]
    .concat();
    asm::to_bytes(r).collect()
}
