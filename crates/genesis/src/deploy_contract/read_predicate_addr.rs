use super::state::*;
use super::state_slot_offset;
use super::tags;
use crate::{deploy_contract::storage_index, utils::state::*};
use essential_state_asm as asm;
use essential_types::Word;

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

fn state_mem_i_not_nil() -> Vec<asm::Op> {
    ops![
        REPEAT_COUNTER,
        VALUE_LEN,
        PUSH: 0,
        EQ,
        NOT,
    ]
}

fn state_mem_i_store_words(num: Word) -> Vec<asm::Op> {
    ops![
        PUSH: num,
        REPEAT_COUNTER,
        STORE,
    ]
}

fn state_mem_i_clear() -> Vec<asm::Op> {
    ops![REPEAT_COUNTER, CLEAR,]
}

fn body() -> Vec<asm::Op> {
    [
        ops![
            PUSH: storage_index::PREDICATES,
        ],
        predicate_addrs_i_address(),
        alloc(1),
        single_key_at_counter(5),
        predicate_addrs_i_tag(),
        state_mem_i_not_nil(),
        state_mem_i_clear(),
        state_mem_i_store_words(2),
    ]
    .concat()
}

pub fn read_predicate_addr() -> Vec<u8> {
    let r = [
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
        // EXISTING_TAG
        match_asm_tag(predicate_addrs_i_tag(), tags::EXISTING, body()),
        //
        // No match
        panic_on_no_match_asm_tag(predicate_addrs_i_tag()),
        // Loop end
        ops![REPEAT_END],
    ]
    .concat();
    asm::to_bytes(r).collect()
}
