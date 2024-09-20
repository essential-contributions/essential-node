use super::{constraint as c, tags};
pub use asm::short::*;
use essential_constraint_asm as c_asm;
use essential_state_asm as asm;
use essential_types::Word;

pub fn to_state(ops: Vec<c_asm::Op>) -> Vec<asm::Op> {
    ops.into_iter().map(asm::Op::from).collect()
}

pub fn push_predicate_i() -> Vec<asm::Op> {
    to_state(c::push_predicate_i())
}

pub fn read_dec_var(slot_ix: Word, value_ix: Word, len: Word) -> Vec<asm::Op> {
    to_state(c::read_dec_var(slot_ix, value_ix, len))
}

pub fn read_predicate_tag() -> Vec<asm::Op> {
    to_state(c::read_predicate_tag())
}

pub fn read_predicate_padding_len() -> Vec<asm::Op> {
    to_state(c::read_predicate_padding_len())
}

pub fn read_predicate_len() -> Vec<asm::Op> {
    to_state(c::read_predicate_len())
}

pub fn read_predicate_words_len() -> Vec<asm::Op> {
    to_state(c::read_predicate_words_len())
}

pub fn read_predicate_words() -> Vec<asm::Op> {
    to_state(c::read_predicate_words())
}

pub fn read_predicate_size() -> Vec<asm::Op> {
    to_state(c::read_predicate_size())
}

pub fn read_salt() -> Vec<asm::Op> {
    to_state(c::read_salt())
}

pub fn predicate_addrs_i() -> Vec<asm::Op> {
    to_state(c::predicate_addrs_i())
}

pub fn predicate_addrs_i_address() -> Vec<asm::Op> {
    to_state(c::predicate_addrs_i_address())
}

pub fn predicate_addrs_i_tag() -> Vec<asm::Op> {
    to_state(c::predicate_addrs_i_tag())
}

pub fn alloc(amount: Word) -> Vec<asm::Op> {
    vec![PUSH(amount), SALC]
}

pub fn single_key(key_len: Word, slot_ix: Word) -> Vec<asm::Op> {
    vec![PUSH(key_len), PUSH(1), PUSH(slot_ix), KRNG]
}

pub fn single_key_at_counter(key_len: Word) -> Vec<asm::Op> {
    vec![PUSH(key_len), PUSH(1), REPC, KRNG]
}

pub fn slot_size(slot_ix: Word) -> Vec<asm::Op> {
    vec![PUSH(slot_ix), SVLEN]
}

pub fn clear_slot(slot_ix: Word) -> Vec<asm::Op> {
    vec![PUSH(slot_ix), SCLR]
}

pub fn clear_last_slot() -> Vec<asm::Op> {
    vec![SCLR, PUSH(1), SUB, SCLR]
}

pub fn store_state_slot(len: Word, slot_ix: Word) -> Vec<asm::Op> {
    vec![PUSH(len), PUSH(slot_ix), SSTR]
}

pub fn store_last_state_slot(len: Word) -> Vec<asm::Op> {
    vec![PUSH(len), SCLR, PUSH(1), SUB, SSTR]
}

pub fn extend_storage_mem(len: Word, slot_ix: Word) -> Vec<asm::Op> {
    vec![PUSH(len), PUSH(slot_ix), SEXT]
}

pub fn value_len_state_slot(slot_ix: Word) -> Vec<asm::Op> {
    vec![PUSH(slot_ix), SVLEN]
}

pub fn load_state_slot(slot_ix: Word, value_ix: Word, len: Word) -> Vec<asm::Op> {
    vec![PUSH(slot_ix), PUSH(value_ix), PUSH(len), SLD]
}

pub fn load_all_state_slot(slot_ix: Word) -> Vec<asm::Op> {
    vec![PUSH(slot_ix), PUSH(0), PUSH(slot_ix), SVLEN, SLD]
}

pub fn load_last_state_slot(value_ix: Word, len: Word) -> Vec<asm::Op> {
    vec![SCLR, PUSH(1), SUB, PUSH(value_ix), PUSH(len), SLD]
}

pub fn match_asm_tag(tag_asm: Vec<asm::Op>, tag: Word, body: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        tag_asm,
        vec![
            PUSH(tag),
            EQ,
            NOT,
            PUSH((body.len() + 1) as Word),
            SWAP,
            JMPIF,
        ],
        body,
    ]
    .concat()
}

pub fn match_tag(tag: Word, body: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        read_predicate_tag(),
        vec![
            PUSH(tag),
            EQ,
            NOT,
            PUSH((body.len() + 1) as Word),
            SWAP,
            JMPIF,
        ],
        body,
    ]
    .concat()
}

pub fn panic_on_no_match_asm_tag(tag: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        tag,
        vec![DUP, PUSH(tags::NUM_TAGS), GTE, SWAP, PUSH(0), LT, OR, PNCIF],
    ]
    .concat()
}

pub fn panic_on_no_match() -> Vec<asm::Op> {
    to_state(c::panic_on_no_match())
}

pub fn debug() -> Vec<asm::Op> {
    to_state(c::debug())
}

pub fn jump_if_cond(cond: Vec<asm::Op>, body: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        cond,
        vec![PUSH((body.len() + 1) as Word), SWAP, JMPIF],
        body,
    ]
    .concat()
}
