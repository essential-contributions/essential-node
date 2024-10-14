use super::dec_var_slot_offset;
use super::predicate_layout_offset;
use super::tags;
use crate::deploy_contract::state_slot_offset_old;
pub use asm::short::*;
use essential_constraint_asm as asm;
use essential_types::Word;

#[cfg(test)]
mod tests;

pub fn push_predicate_i() -> Vec<asm::Op> {
    vec![REPC, PUSH(dec_var_slot_offset::PREDICATES), ADD]
}

pub fn read_dec_var(slot_ix: Word, value_ix: Word, len: Word) -> Vec<asm::Op> {
    vec![PUSH(slot_ix), PUSH(value_ix), PUSH(len), VAR]
}

pub fn read_predicate_tag() -> Vec<asm::Op> {
    [
        push_predicate_i(),
        vec![PUSH(predicate_layout_offset::TAG), PUSH(1), VAR],
    ]
    .concat()
}

pub fn read_predicate_len() -> Vec<asm::Op> {
    [push_predicate_i(), vec![VLEN]].concat()
}

pub fn read_predicate_bytes_len() -> Vec<asm::Op> {
    [
        push_predicate_i(),
        vec![PUSH(predicate_layout_offset::BYTES_LEN), PUSH(1), VAR],
    ]
    .concat()
}

pub fn read_predicate_words_len() -> Vec<asm::Op> {
    [
        read_predicate_len(),
        vec![PUSH(predicate_layout_offset::WORDS), SUB],
    ]
    .concat()
}

pub fn read_predicate_words() -> Vec<asm::Op> {
    [
        push_predicate_i(),
        vec![PUSH(predicate_layout_offset::WORDS)],
        read_predicate_words_len(),
        vec![VAR],
    ]
    .concat()
}

pub fn read_predicate_word_i(i: Word) -> Vec<asm::Op> {
    [
        push_predicate_i(),
        vec![PUSH(predicate_layout_offset::WORDS), PUSH(i), ADD, PUSH(1)],
        vec![VAR],
    ]
    .concat()
}

pub fn read_predicate_size() -> Vec<asm::Op> {
    read_dec_var(dec_var_slot_offset::NUM_PREDICATES, 0, 1)
}

pub fn read_salt() -> Vec<asm::Op> {
    read_dec_var(dec_var_slot_offset::SALT, 0, 4)
}

pub fn predicate_addrs_i() -> Vec<asm::Op> {
    vec![PUSH(state_slot_offset_old::PREDICATE_ADDRS), REPC, PUSH(5), MUL]
}

pub fn predicate_addrs_i_address() -> Vec<asm::Op> {
    [
        predicate_addrs_i(),
        vec![PUSH(1), ADD, PUSH(4), PUSH(0), STATE],
    ]
    .concat()
}

pub fn predicate_addrs_i_tag() -> Vec<asm::Op> {
    [predicate_addrs_i(), vec![PUSH(1), PUSH(0), STATE]].concat()
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
    [
        read_predicate_tag(),
        vec![DUP, PUSH(tags::NUM_TAGS), GTE, SWAP, PUSH(0), LT, OR, PNCIF],
    ]
    .concat()
}

pub fn read_state_slot(slot_ix: Word, value_ix: Word, len: Word, delta: bool) -> Vec<asm::Op> {
    vec![
        PUSH(slot_ix),
        PUSH(value_ix),
        PUSH(len),
        PUSH(delta as Word),
        STATE,
    ]
}

pub fn state_slot_len(slot_ix: Word, delta: bool) -> Vec<asm::Op> {
    vec![PUSH(slot_ix), PUSH(delta as Word), SLEN]
}

pub fn state_slot_is_nil(slot_ix: Word, delta: bool) -> Vec<asm::Op> {
    [state_slot_len(slot_ix, delta), vec![PUSH(0), EQ]].concat()
}

pub fn jump_if(body: Vec<asm::Op>) -> Vec<asm::Op> {
    [vec![PUSH((body.len() + 1) as Word), SWAP, JMPIF], body].concat()
}

pub fn jump_if_cond(cond: Vec<asm::Op>, body: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        cond,
        vec![PUSH((body.len() + 1) as Word), SWAP, JMPIF],
        body,
    ]
    .concat()
}

pub fn debug() -> Vec<asm::Op> {
    vec![PUSH(1), PNCIF]
}

pub fn debug_i(i: Word) -> Vec<asm::Op> {
    vec![REPC, PUSH(i), EQ, PNCIF]
}
