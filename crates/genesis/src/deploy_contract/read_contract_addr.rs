use super::state::*;
use super::state_slot_offset;
use super::tags;
use crate::{deploy_contract::storage_index, utils::state::*};
use essential_state_asm as asm;

fn new_tag_body() -> Vec<asm::Op> {
    [
        read_predicate_words(),
        read_predicate_words_len(),
        read_predicate_padding_len(),
        ops![SHA_256],
    ]
    .concat()
}

fn write_tag_and_address() -> Vec<asm::Op> {
    [
        jump_if_cond(
            ops![
                REPEAT_COUNTER,
                PUSH: 0,
                EQ,
            ],
            load_all_state_slot(state_slot_offset::PREDICATE_ADDRS),
        ),
        ops![
            REPEAT_COUNTER,
            PUSH: 5,
            MUL,
            PUSH: 4,
            ADD,
            PUSH: state_slot_offset::PREDICATE_ADDRS,
            STORE,
        ],
        read_predicate_tag(),
        load_all_state_slot(state_slot_offset::PREDICATE_ADDRS),
        ops![
            REPEAT_COUNTER,
            PUSH: 5,
            MUL,
            PUSH: 5,
            ADD,
            PUSH: state_slot_offset::PREDICATE_ADDRS,
            STORE,
        ],
        load_state_slot(state_slot_offset::PREDICATE_ADDRS, 1, 4),
    ]
    .concat()
}

pub fn read_contract_addr() -> Vec<u8> {
    let r = [
        ops![
            PUSH: storage_index::CONTRACTS,
        ],
        alloc(3),
        // for i in 0..predicates_size
        read_predicate_size(),
        ops![
            PUSH: 1,
            REPEAT,
        ],
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
        write_tag_and_address(),
        //
        // loop end
        ops![REPEAT_END,],
        // salt
        //
        read_salt(),
        //
        // sha256([predicate_hashes..., salt])
        read_predicate_size(),
        ops![
            PUSH: 4,
            MUL,
            PUSH: 4,
            ADD,
            PUSH: 0,
            SHA_256,
        ],
        // Write contract_addr as set to storage
        store_state_slot(4, state_slot_offset::CONTRACT_ADDR),
        load_state_slot(state_slot_offset::CONTRACT_ADDR, 0, 4),
        //
        // state deployed_contract = storage::contracts[hash];
        single_key(5, state_slot_offset::CONTRACT_EXISTS),
    ]
    .concat();
    asm::to_bytes(r).collect()
}
