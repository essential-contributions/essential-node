use super::state::*;
use super::state_mem_offset;
use super::tags;
use crate::deploy_contract::storage_index;
use essential_state_asm as asm;
use essential_types::Word;

#[cfg(test)]
mod tests;

fn new_tag_body() -> Vec<asm::Op> {
    [
        read_predicate_words(),
        read_predicate_bytes_len(),
        vec![SHA2],
    ]
    .concat()
}

/// # Args
///
/// # Accesses
///  Repeat counter
///
/// # Returns
/// State memory 4 words
fn load_last_addr() -> Vec<asm::Op> {
    vec![
        PUSH(state_mem_offset::PREDICATE_ADDRS),
        REPC,
        PUSH(5),
        MUL,
        PUSH(1),
        ADD,
        PUSH(4),
        LODS,
    ]
}

fn repeat(num: Word, count_up: bool, ops: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        vec![PUSH(num), PUSH(count_up as Word), REP],
        ops,
        vec![REPE],
    ]
    .concat()
}

fn repeat_num(mut num: Vec<asm::Op>, count_up: bool, ops: Vec<asm::Op>) -> Vec<asm::Op> {
    let ops = repeat(0, count_up, ops);
    num.extend_from_slice(&ops[1..]);
    num
}

fn body_setup() -> Vec<asm::Op> {
    [
        vec![PUSH(state_mem_offset::PREDICATE_ADDRS), DUP, SMVLEN],
        read_predicate_tag(),
    ]
    .concat()
}

fn repeat_body() -> Vec<asm::Op> {
    [
        body_setup(),
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
        vec![PUSH(5), STOS],
        load_last_addr(),
        //
        // loop end
    ]
    .concat()
}

pub fn read_contract_addr() -> Vec<u8> {
    let r = [
        vec![PUSH(storage_index::CONTRACTS)],
        alloc(3),
        // for i in 0..predicates_size
        repeat_num(read_predicate_size(), true, repeat_body()),
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
        read_single_key(5, state_mem_offset::CONTRACT_EXISTS),
    ]
    .concat();
    asm::to_bytes(r).collect()
}
