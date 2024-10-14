use super::{constraint::*, tags};
use crate::deploy_contract::{dec_var_slot_offset, predicate_layout_offset, state_slot_offset};
use essential_constraint_asm as asm;
use essential_types::{contract::Contract, predicate::Predicate, Word};

#[cfg(test)]
mod tests;

/// TODO: Move this to `essential-types::Contract::MAX_BYTES`
/// 100 KB
pub const MAX_CONTRACT_BYTES: Word = 1024 * 100;

pub fn check_max_bytes() -> Vec<u8> {
    asm::to_bytes(check_max()).collect()
}
fn check_max() -> Vec<asm::Op> {
    [
        vec![
            // Alloc temp for total
            PUSH(1),
            ALOCT,
            POP,
            // base case
            PUSH(true as Word),
            PUSH(1),
        ],
        for_loop(
            [
                // total += __var_len(i + 2);
                inc_total(),
                check_max_predicate_bytes(),
                match_tag(
                    tags::NEW,
                    [
                        check_max_state_reads(),
                        check_max_constraints(),
                        max_state_inner_loop(check_max_state_read_size()),
                        max_constraint_inner_loop(check_max_constraint_size()),
                    ]
                    .concat(),
                ),
                match_tag(tags::EXISTING, vec![PUSH(true as Word), AND]),
                panic_on_no_match(),
            ]
            .concat(),
        ),
    ]
    .concat()
}

fn max_constraint_inner_loop(check_max_constraint_size: Vec<asm::Op>) -> Vec<asm::Op> {
    todo!()
}

fn max_state_inner_loop(
    check_max_state_read_size: Vec<asm::Op>,
) -> Vec<essential_constraint_asm::Op> {
    todo!()
}

fn push_num_predicates() -> Vec<asm::Op> {
    vec![
        PUSH(dec_var_slot_offset::NUM_PREDICATES), // slot_ix
        PUSH(0),                                   // value_ix
        PUSH(1),                                   // len
        VAR,
    ]
}

/// # Args
/// index into hashes
fn push_hash() -> Vec<asm::Op> {
    vec![
        PUSH(state_slot_offset::GENERATE_HASHES), // slot_ix
        // value_ix
        SWAP,
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
        vec![PUSH(true as Word), REP],
        body,
        vec![REPE],
    ]
    .concat()
}

/// __var_len(i + 2) <= MAX_PREDICATE_BYTES
fn check_max_predicate_bytes() -> Vec<asm::Op> {
    vec![
        // slot_ix
        PUSH(dec_var_slot_offset::PREDICATES),
        REPC,
        ADD,
        VLEN,
        PUSH(Predicate::MAX_BYTES as Word),
        ADD,
        AND,
    ]
}

/// __read(bytes, 1, 0, 1) <= MAX_STATE_READS
fn check_max_state_reads() -> Vec<asm::Op> {
    [
        push_static_header_word(),
        vec![PUSH(7)],
        read_byte(),
        vec![PUSH(Predicate::MAX_STATE_READS as Word), LTE, AND],
    ]
    .concat()
}

/// __read(bytes, 1, 1, 1) <= MAX_CONSTRAINTS
fn check_max_constraints() -> Vec<asm::Op> {
    [
        push_static_header_word(),
        vec![PUSH(6)],
        read_byte(),
        vec![PUSH(Predicate::MAX_CONSTRAINTS as Word), LTE, AND],
    ]
    .concat()
}

/// __read(bytes, 1, 2 + j * 2, 2) <= MAX_STATE_READ_SIZE
fn check_max_state_read_size() -> Vec<asm::Op> {
    [
        push_dynamic_header_word(),
        push_state_read_len_byte_index(),
        read_byte(),
        vec![PUSH(Predicate::MAX_STATE_READ_SIZE_BYTES as Word), LTE, AND],
    ]
    .concat()
}

fn push_dynamic_header_word() -> Vec<asm::Op> {
    vec![REPC]
}

fn push_state_read_len_byte_index() -> Vec<asm::Op> {
    todo!()
}

/// __read(bytes, 1, 2 + j * 2 * __read(bytes, 1, 0, 1), 2) <= MAX_CONSTRAINT_SIZE
fn check_max_constraint_size() -> Vec<asm::Op> {
    [
        push_dynamic_header_word(),
        push_constraint_len_byte_index(),
        read_byte(),
        vec![PUSH(Predicate::MAX_CONSTRAINT_SIZE_BYTES as Word), LTE, AND],
    ]
    .concat()
}

fn push_constraint_len_byte_index() -> Vec<asm::Op> {
    todo!()
}

/// total <= MAX_CONTRACT_BYTES
fn check_max_contract_bytes() -> Vec<asm::Op> {
    [
        load_total(),
        vec![PUSH(MAX_CONTRACT_BYTES as Word), LTE, AND],
    ]
    .concat()
}

fn load_total() -> Vec<asm::Op> {
    vec![PUSH(0), LOD]
}

fn inc_total() -> Vec<asm::Op> {
    [
        load_total(),
        vec![
            REPC,
            PUSH(dec_var_slot_offset::PREDICATES),
            ADD,
            VLEN,
            ADD,
            PUSH(0),
            STO,
        ],
    ]
    .concat()
}
fn push_static_header_word() -> Vec<asm::Op> {
    vec![
        // slot_ix
        PUSH(dec_var_slot_offset::PREDICATES),
        REPC,
        ADD,
        PUSH(predicate_layout_offset::WORDS), // value_ix
        PUSH(1),                              // len
        VAR,
    ]
}

/// # Args
/// Word
/// Index
fn read_byte() -> Vec<asm::Op> {
    vec![
        // Convert index to bits
        PUSH(8),
        MUL,
        DUP,        // [Word, Index, Index]
        PUSH(0xFF), // [Word, Index, Index, 0xFF]
        SWAP,       // [Word, Index, 0xFF, Index]
        SHL,        // [Word, Index, 0xFF << Index]
        SWAP,       // [Word, 0xFF << Index, Index]
        PUSH(2),
        SWAPI, // [Index, 0xFF << Index, Word]
        BAND,  // [Index, Word & (0xFF << Index)]
        SWAP,  // [Word & (0xFF << Index), Index]
        SHR,
    ]
}
