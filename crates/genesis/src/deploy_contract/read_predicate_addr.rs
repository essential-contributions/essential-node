use super::state::*;
use super::tags;
use crate::deploy_contract::storage_index;
use essential_state_asm as asm;
use essential_types::Word;

fn state_mem_i_not_nil() -> Vec<asm::Op> {
    vec![REPC, SMVLEN, PUSH(0), EQ, NOT]
}

fn state_mem_i_store_words(num: Word) -> Vec<asm::Op> {
    vec![PUSH(num), REPC, STOS]
}

fn state_mem_i_clear() -> Vec<asm::Op> {
    vec![REPC, TRUNC]
}

fn body() -> Vec<asm::Op> {
    [
        vec![PUSH(storage_index::PREDICATES)],
        predicate_addrs_i_address(),
        alloc(1),
        read_single_key_counter_slot(5),
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
        vec![PUSH(1), REP],
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
        vec![REPE],
    ]
    .concat();
    asm::to_bytes(r).collect()
}
