use super::state::*;
use super::state_mem_offset;
use super::tags;
use crate::deploy_contract::storage_index;
use essential_state_asm as asm;

fn new_tag_body() -> Vec<asm::Op> {
    [
        jump_if_cond(
            vec![PUSH(1), NSLT, PUSH(3), EQ],
            [
                read_predicate_words(),
                read_predicate_words_len(),
                read_predicate_padding_len(),
                vec![SHA2],
            ]
            .concat(),
        ),
        jump_if_cond(
            vec![PUSH(1), NSLT, PUSH(3), EQ, NOT],
            predicate_addrs_i_address(),
        ),
    ]
    .concat()
}

fn load_last_addr() -> Vec<asm::Op> {
    vec![
        PUSH(state_mem_offset::PREDICATE_ADDRS),
        REPC,
        PUSH(5),
        MUL,
        PUSH(1),
        ADD,
        PUSH(4),
        SLD,
    ]
}

pub fn read_contract_addr() -> Vec<u8> {
    let r = [
        vec![PUSH(storage_index::CONTRACTS)],
        alloc(3),
        // for i in 0..predicates_size
        read_predicate_size(),
        vec![PUSH(1), REP],
        read_predicate_tag(),
        // match tag
        // NEW_TAG
        match_tag(tags::NEW, new_tag_body()),
        //
        // EXISTING_TAG
        match_tag(tags::EXISTING, read_predicate_words()),
        //
        // No match
        panic_on_no_match(),
        // Write tag and predicate address to storage
        extend_storage_mem(5, state_mem_offset::PREDICATE_ADDRS),
        load_last_addr(),
        //
        // loop end
        vec![REPE],
        // salt
        //
        read_salt(),
        //
        // sha256([predicate_hashes..., salt])
        read_predicate_size(),
        vec![PUSH(4), MUL, PUSH(4), ADD, PUSH(0), SHA2],
        // Write contract_addr as set to storage
        store_state_slot(4, state_mem_offset::CONTRACT_ADDR),
        load_state_slot(state_mem_offset::CONTRACT_ADDR, 0, 4),
        //
        // state deployed_contract = storage::contracts[hash];
        single_key(5, state_mem_offset::CONTRACT_EXISTS),
    ]
    .concat();
    asm::to_bytes(r).collect()
}
