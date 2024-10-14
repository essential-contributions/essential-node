use crate::deploy_contract::dec_var_slot_offset;
use crate::deploy_contract::predicate_layout_offset;

use super::state::*;
use super::tags;
use essential_state_asm as asm;
use essential_types::Word;

#[cfg(test)]
mod tests;

const SIZE_OF_TAG_HASH: Word = 5;

pub fn generate_hashes_bytes() -> Vec<u8> {
    asm::to_bytes(generate_hashes()).collect()
}

fn generate_hashes() -> Vec<asm::Op> {
    [
        vec![PUSH(1), ALOCS], // Allocate the slot for this state
        push_num_predicates(),
        contract_storage_location(),
        for_loop(
            [
                match_tag(tags::NEW, hash_new_body()),
                match_tag(tags::EXISTING, existing_body()),
                panic_on_no_match(),
            ]
            .concat(),
        ),
        hash_contract(),
    ]
    .concat()
}

fn for_loop(body: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        push_num_predicates(),
        vec![PUSH(true as Word), REP],
        body,
        vec![REPE],
    ]
    .concat()
}

fn hash_new_body() -> Vec<asm::Op> {
    [
        storage_location(),
        vec![PUSH(tags::NEW)],
        push_predicate_bytes_word_len(),
        push_predicate_bytes(),
        push_predicate_bytes_byte_len(),
        vec![
            SHA2, // Hash the predicate bytes
            // Store tag and hash
            PUSH(SIZE_OF_TAG_HASH),
            STOS,
        ],
        load_predicate_hash(),
    ]
    .concat()
}

fn existing_body() -> Vec<asm::Op> {
    [
        storage_location(),
        vec![PUSH(tags::EXISTING)],
        push_predicate_bytes_word_len(),
        push_predicate_bytes(),
        vec![
            // Store tag and hash
            PUSH(SIZE_OF_TAG_HASH),
            STOS,
        ],
        load_predicate_hash(),
    ]
    .concat()
}

fn hash_contract() -> Vec<asm::Op> {
    [
        push_salt(),
        push_num_predicates(),
        vec![
            // hash and salt size in bytes
            PUSH(4),
            MUL,
            PUSH(4),
            ADD,
            PUSH(8),
            MUL,
            SHA2,    // Hash the salt and predicate hashes
            PUSH(4), // len
            STOS,    // Store the contract hash
        ],
    ]
    .concat()
}

/// # Args
/// Number of predicates
fn contract_storage_location() -> Vec<asm::Op> {
    vec![
        PUSH(0), // slot_ix
        SWAP,    // [slot_ix, num_predicates]
        // value_ix
        PUSH(SIZE_OF_TAG_HASH),
        MUL,
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

fn push_salt() -> Vec<asm::Op> {
    vec![
        PUSH(dec_var_slot_offset::SALT), // slot_ix
        PUSH(0),                         // value_ix
        PUSH(4),                         // len
        VAR,
    ]
}

fn storage_location() -> Vec<asm::Op> {
    vec![
        PUSH(0), // slot_ix
        // value_ix
        REPC,
        PUSH(SIZE_OF_TAG_HASH),
        MUL,
    ]
}

fn push_predicate_bytes_byte_len() -> Vec<asm::Op> {
    vec![
        // slot_ix
        PUSH(dec_var_slot_offset::PREDICATES),
        REPC,
        ADD,
        PUSH(predicate_layout_offset::BYTES_LEN), // value_ix
        PUSH(1),                                  // len
        VAR,
    ]
}

fn push_predicate_bytes_word_len() -> Vec<asm::Op> {
    vec![
        // slot_ix
        PUSH(dec_var_slot_offset::PREDICATES),
        REPC,
        ADD,
        VLEN, // Get total len
        // Subtract the tag and bytes len
        PUSH(predicate_layout_offset::WORDS),
        SUB,
    ]
}

/// # Args
/// Predicate bytes word len
fn push_predicate_bytes() -> Vec<asm::Op> {
    vec![
        // slot_ix
        PUSH(dec_var_slot_offset::PREDICATES),
        REPC,
        ADD,
        SWAP,                                 // [slot_ix, len]
        PUSH(predicate_layout_offset::WORDS), // value_ix
        SWAP,                                 // [slot_ix, value_ix, len]
        VAR,
    ]
}

fn load_predicate_hash() -> Vec<asm::Op> {
    vec![
        PUSH(0), // slot_ix
        // value_ix
        PUSH(SIZE_OF_TAG_HASH),
        REPC,
        MUL,
        PUSH(1),
        ADD,
        PUSH(4), // len
        LODS,
    ]
}
