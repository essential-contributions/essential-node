use std::ops;

use super::constraint::*;
use super::dec_var_slot_offset;
use super::predicate_layout_offset;
use super::state_slot_offset;
use super::tags;
use crate::{deploy_contract::storage_index, utils::constraint::*};
use essential_check as check;
use essential_constraint_asm as asm;
use essential_types::predicate::Predicate;
use essential_types::Word;

fn check_num_predicates() -> Vec<asm::Op> {
    [
        read_predicate_size(),
        ops![
            DUP,
            PUSH: check::predicate::MAX_PREDICATES as Word,
            LTE,
            SWAP,
            PUSH: 2,
            ADD,
            PUSH: 0,
            NUM_SLOTS,
            EQ,
            AND,
        ],
    ]
    .concat()
}

fn read_num_state_reads() -> Vec<asm::Op> {
    [read_predicate_word_i(0), read_byte(7)].concat()
}

fn read_num_constraints() -> Vec<asm::Op> {
    [read_predicate_word_i(0), read_byte(6)].concat()
}

fn read_bytes(range: std::ops::Range<usize>) -> Vec<asm::Op> {
    assert!(range.end < 8);
    let mut mask: i64 = 0;
    for i in range.clone() {
        mask |= 0xFF << (i * 8);
    }
    ops![
        PUSH: mask,
        BIT_AND,
        PUSH: range.start as Word * 8,
        SHR,
    ]
}

fn read_byte(byte: Word) -> Vec<asm::Op> {
    ops![
        PUSH: 0xFF << (byte * 8),
        BIT_AND,
        PUSH: byte * 8,
        SHR,
    ]
}

/// Offset
/// Word
fn read_len() -> Vec<asm::Op> {
    [
        ops![
            DUP, // Copy the word
            PUSH: 7,
            REPEAT_COUNTER, // [Offset, Word, Word, 7, Counter]
            PUSH: 4,
            DUP_FROM,
            ADD,
            PUSH: 4,
            MOD,
            PUSH: 2,
            MUL,
            PUSH: 1,
            ADD,
            SUB,
        ], // [Word, Index]
        read_byte_asm(), // [Word, byte_0]
        ops![
            SWAP, // [byte_0, Word]
            PUSH: 7,
            REPEAT_COUNTER,
            PUSH: 4,
            DUP_FROM,
            ADD,
            PUSH: 4,
            MOD,
            PUSH: 2,
            MUL,
            SUB,
        ], // [byte_0, Word, Index]
        read_byte_asm(), // [byte_0, byte_1]
        ops![
            PUSH: 8, // [byte_0, byte_1, 8]
            SHL, // [byte_0, byte_1 << 8]
            BIT_OR,
            SWAP,
            POP,
        ],
    ]
    .concat()
}

fn check_num_state_reads() -> Vec<asm::Op> {
    [
        read_num_state_reads(),
        ops![
            PUSH: Predicate::MAX_STATE_READS as Word,
            LTE,
            AND,
        ],
    ]
    .concat()
}

// 0 => 0
// 1 => 1
// 2 => 1
// 3 => 2
// 4 => 2
// 5 => 3
// (i + 1) / 4
fn calc_state_word() -> Vec<asm::Op> {
    ops![
        REPEAT_COUNTER,
        PUSH: 1,
        ADD,
        PUSH: 4,
        DIV,
    ]
}

/// (i + (1 + num_state_reads)) / 4
///
/// num_state_reads
fn calc_constraint_word() -> Vec<asm::Op> {
    ops![
        REPEAT_COUNTER,
        PUSH: 1,
        ADD,
        ADD,
        PUSH: 4,
        DIV,
    ]
}

/// Word
/// Index
fn read_byte_asm() -> Vec<asm::Op> {
    ops![
        PUSH: 8,
        MUL,
        DUP, // [Word, Index, Index]
        PUSH: 2,
        SWAP_INDEX, // [Index, Index, Word]
        SWAP, // [Index, Word, Index]
        PUSH: 0xFF, // [Index, Word, Index, 0xFF]
        SWAP, // [Index, Word, 0xFF, Index]
        SHL, // [Index, Word, 0xFF << Index]
        BIT_AND, // [Index, Word & (0xFF << Index)]
        SWAP,
        SHR,
    ]
}

fn check_num_constraints() -> Vec<asm::Op> {
    [
        read_num_constraints(),
        ops![
            PUSH: Predicate::MAX_CONSTRAINTS as Word,
            LTE,
            AND,
        ],
    ]
    .concat()
}

fn check_state_read_sizes() -> Vec<asm::Op> {
    [
        ops![
            PUSH: true as Word, // Fold init
            REPEAT_COUNTER,
        ],
        read_num_state_reads(),
        ops![
            PUSH: true as Word, // Count up
            REPEAT,
        ],
        ops![
            DUP, // [bool, i, i], [bool, i, i]
            PUSH: dec_var_slot_offset::PREDICATES,
            ADD,
            PUSH: predicate_layout_offset::WORDS,
        ],
        calc_state_word(),
        ops![
            ADD,
            PUSH: 1,
            DECISION_VAR,
            PUSH: 1, // read len Offset
            SWAP,
        ], // [bool, i, word]
        read_len(), // [bool, i, len]
        sum_total_bytes(),
        ops![
            PUSH: Predicate::MAX_STATE_READ_SIZE_BYTES as Word,
            LTE, // [bool, i, len <= MAX_STATE_READ_SIZE_BYTES]
            SWAP, // [bool, len <= MAX_STATE_READ_SIZE_BYTES, i]
            PUSH: 2,
            SWAP_INDEX, // [i, len <= MAX_STATE_READ_SIZE_BYTES, bool]
            AND, // [i, bool]
            SWAP, // [bool, i]
        ],
        ops![REPEAT_END, POP, AND],
    ]
    .concat()
}

fn sum_total_bytes() -> Vec<asm::Op> {
    ops![
        DUP,
        PUSH: 0,
        TEMP_LOAD,
        ADD,
        PUSH: 0,
        SWAP,
        TEMP_STORE,
    ]
}

/// bool
fn check_constraint_sizes() -> Vec<asm::Op> {
    [
        read_num_state_reads(),
        ops![
            PUSH: true as Word, // Fold init
            REPEAT_COUNTER,
        ],
        read_num_constraints(),
        ops![
            PUSH: true as Word, // Count up
            REPEAT,
        ],
        ops![
            DUP, // [num_state_reads, bool, i, i]
            PUSH: dec_var_slot_offset::PREDICATES, // [nsr, bool, i, i, PREDICATES]
            ADD, // [nsr, bool, i, i + PREDICATES]
            PUSH: predicate_layout_offset::WORDS, // [nsr, bool, i, i + PREDICATES, WORDS]
            PUSH: 4,
            DUP_FROM,
        ], // [num_state_reads]
        calc_constraint_word(),
        ops![
            ADD,
            PUSH: 1,
            DECISION_VAR, // [nsr, bool, i, word]
            PUSH: 3,
            DUP_FROM,
            PUSH: 1,
            ADD,
            SWAP,
        ],
        read_len(), // [bool, i, len]
        sum_total_bytes(),
        ops![
            PUSH: Predicate::MAX_CONSTRAINT_SIZE_BYTES as Word,
            LTE, // [bool, i, len <= MAX_STATE_READ_SIZE_BYTES]
            SWAP, // [bool, len <= MAX_STATE_READ_SIZE_BYTES, i]
            PUSH: 2,
            SWAP_INDEX, // [i, len <= MAX_STATE_READ_SIZE_BYTES, bool]
            AND, // [i, bool]
            SWAP, // [nsr, bool, i]
        ],
        ops![REPEAT_END, POP, SWAP, POP, AND],
    ]
    .concat()
}

fn check() -> Vec<asm::Op> {
    [
        check_num_state_reads(),
        check_num_constraints(),
        // TODO: Check encoded length is positive
        check_state_read_sizes(),
        // TODO: Check encoded length is positive
        check_constraint_sizes(),
        read_num_state_reads(),
        read_num_constraints(),
        ops![
            ADD,
            PUSH: 2,
            MUL,
            PUSH: 2,
            ADD,
            PUSH: 0,
            TEMP_LOAD,
            ADD,
            PUSH: Predicate::MAX_BYTES as Word,
            LTE,
            AND,
        ],
    ]
    .concat()
}

pub fn validate_contract() -> Vec<u8> {
    let r = [
        check_num_predicates(),
        read_predicate_size(),
        ops![
            PUSH: 1,
            TEMP_ALLOC,
            POP,
            PUSH: 1,
            REPEAT
        ],
        match_tag(tags::NEW, check()),
        ops![REPEAT_END],
    ]
    .concat();
    asm::to_bytes(r).collect()
}
