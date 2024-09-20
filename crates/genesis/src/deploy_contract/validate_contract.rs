use super::constraint::*;
use super::dec_var_slot_offset;
use super::predicate_layout_offset;
use super::tags;
use essential_check as check;
use essential_constraint_asm as asm;
use essential_types::predicate::Predicate;
use essential_types::Word;

fn check_num_predicates() -> Vec<asm::Op> {
    [
        read_predicate_size(),
        vec![
            DUP,
            PUSH(check::predicate::MAX_PREDICATES as Word),
            LTE,
            SWAP,
            PUSH(2),
            ADD,
            PUSH(0),
            NSLT,
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
    vec![PUSH(mask), BAND, PUSH(range.start as Word * 8), SHR]
}

fn read_byte(byte: Word) -> Vec<asm::Op> {
    vec![PUSH(0xFF << (byte * 8)), BAND, PUSH(byte * 8), SHR]
}

/// Offset
/// Word
fn read_len() -> Vec<asm::Op> {
    [
        vec![
            DUP, // Copy the word
            PUSH(7),
            REPC, // [Offset, Word, Word, 7, Counter]
            PUSH(4),
            DUPF,
            ADD,
            PUSH(4),
            MOD,
            PUSH(2),
            MUL,
            PUSH(1),
            ADD,
            SUB,
        ], // [Word, Index]
        read_byte_asm(), // [Word, byte_0]
        vec![
            SWAP, // [byte_0, Word]
            PUSH(7),
            REPC,
            PUSH(4),
            DUPF,
            ADD,
            PUSH(4),
            MOD,
            PUSH(2),
            MUL,
            SUB,
        ], // [byte_0, Word, Index]
        read_byte_asm(), // [byte_0, byte_1]
        vec![
            PUSH(8), // [byte_0, byte_1, 8]
            SHL,     // [byte_0, byte_1 << 8]
            BOR,
            SWAP,
            POP,
        ],
    ]
    .concat()
}

fn check_num_state_reads() -> Vec<asm::Op> {
    [
        read_num_state_reads(),
        vec![PUSH(Predicate::MAX_STATE_READS as Word), LTE, AND],
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
    vec![REPC, PUSH(1), ADD, PUSH(4), DIV]
}

/// (i + (1 + num_state_reads)) / 4
///
/// num_state_reads
fn calc_constraint_word() -> Vec<asm::Op> {
    vec![REPC, PUSH(1), ADD, ADD, PUSH(4), DIV]
}

/// Word
/// Index
fn read_byte_asm() -> Vec<asm::Op> {
    vec![
        PUSH(8),
        MUL,
        DUP, // [Word, Index, Index]
        PUSH(2),
        SWAPI,      // [Index, Index, Word]
        SWAP,       // [Index, Word, Index]
        PUSH(0xFF), // [Index, Word, Index, 0xFF]
        SWAP,       // [Index, Word, 0xFF, Index]
        SHL,        // [Index, Word, 0xFF << Index]
        BAND,       // [Index, Word & (0xFF << Index)]
        SWAP,
        SHR,
    ]
}

fn check_num_constraints() -> Vec<asm::Op> {
    [
        read_num_constraints(),
        vec![PUSH(Predicate::MAX_CONSTRAINTS as Word), LTE, AND],
    ]
    .concat()
}

fn check_state_read_sizes() -> Vec<asm::Op> {
    [
        vec![
            PUSH(true as Word), // Fold init
            REPC,
        ],
        read_num_state_reads(),
        vec![
            PUSH(true as Word), // Count up
            REP,
        ],
        vec![
            DUP, // [bool, i, i], [bool, i, i]
            PUSH(dec_var_slot_offset::PREDICATES),
            ADD,
            PUSH(predicate_layout_offset::WORDS),
        ],
        calc_state_word(),
        vec![
            ADD,
            PUSH(1),
            VAR,
            PUSH(1), // read len Offset
            SWAP,
        ], // [bool, i, word]
        read_len(), // [bool, i, len]
        sum_total_bytes(),
        vec![
            PUSH(Predicate::MAX_STATE_READ_SIZE_BYTES as Word),
            LTE,  // [bool, i, len <= MAX_STATE_READ_SIZE_BYTES]
            SWAP, // [bool, len <= MAX_STATE_READ_SIZE_BYTES, i]
            PUSH(2),
            SWAPI, // [i, len <= MAX_STATE_READ_SIZE_BYTES, bool]
            AND,   // [i, bool]
            SWAP,  // [bool, i]
        ],
        vec![REPE, POP, AND],
    ]
    .concat()
}

fn sum_total_bytes() -> Vec<asm::Op> {
    vec![DUP, PUSH(0), TLD, ADD, PUSH(0), SWAP, TSTR]
}

/// bool
fn check_constraint_sizes() -> Vec<asm::Op> {
    [
        read_num_state_reads(),
        vec![
            PUSH(true as Word), // Fold init
            REPC,
        ],
        read_num_constraints(),
        vec![
            PUSH(true as Word), // Count up
            REP,
        ],
        vec![
            DUP,                                   // [num_state_reads, bool, i, i]
            PUSH(dec_var_slot_offset::PREDICATES), // [nsr, bool, i, i, PREDICATES]
            ADD,                                   // [nsr, bool, i, i + PREDICATES]
            PUSH(predicate_layout_offset::WORDS),  // [nsr, bool, i, i + PREDICATES, WORDS]
            PUSH(4),
            DUPF,
        ], // [num_state_reads]
        calc_constraint_word(),
        vec![
            ADD,
            PUSH(1),
            VAR, // [nsr, bool, i, word]
            PUSH(3),
            DUPF,
            PUSH(1),
            ADD,
            SWAP,
        ],
        read_len(), // [bool, i, len]
        sum_total_bytes(),
        vec![
            PUSH(Predicate::MAX_CONSTRAINT_SIZE_BYTES as Word),
            LTE,  // [bool, i, len <= MAX_STATE_READ_SIZE_BYTES]
            SWAP, // [bool, len <= MAX_STATE_READ_SIZE_BYTES, i]
            PUSH(2),
            SWAPI, // [i, len <= MAX_STATE_READ_SIZE_BYTES, bool]
            AND,   // [i, bool]
            SWAP,  // [nsr, bool, i]
        ],
        vec![REPE, POP, SWAP, POP, AND],
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
        vec![
            ADD,
            PUSH(2),
            MUL,
            PUSH(2),
            ADD,
            PUSH(0),
            TLD,
            ADD,
            PUSH(Predicate::MAX_BYTES as Word),
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
        vec![PUSH(1), TALC, POP, PUSH(1), REP],
        match_tag(tags::NEW, check()),
        vec![REPE],
    ]
    .concat();
    asm::to_bytes(r).collect()
}
