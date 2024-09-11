use super::dec_var_slot_offset;
use super::predicate_layout_offset;
use super::tags;
use crate::utils::constraint::*;
use essential_constraint_asm as asm;
use essential_types::Word;

pub fn push_predicate_i() -> Vec<asm::Op> {
    ops![
        REPEAT_COUNTER,
        PUSH: dec_var_slot_offset::PREDICATES,
        ADD,
    ]
}

pub fn read_dec_var(slot_ix: Word, value_ix: Word, len: Word) -> Vec<asm::Op> {
    ops![
        PUSH: slot_ix,
        PUSH: value_ix,
        PUSH: len,
        DECISION_VAR,
    ]
}

pub fn read_predicate_tag() -> Vec<asm::Op> {
    [
        push_predicate_i(),
        ops![
            PUSH: predicate_layout_offset::TAG,
            PUSH: 1,
            DECISION_VAR,
        ],
    ]
    .concat()
}

pub fn read_predicate_len() -> Vec<asm::Op> {
    [push_predicate_i(), ops![DECISION_VAR_LEN,]].concat()
}

pub fn read_predicate_padding_len() -> Vec<asm::Op> {
    [
        push_predicate_i(),
        ops![
            PUSH: predicate_layout_offset::PADDING_LEN,
            PUSH: 1,
            DECISION_VAR,
        ],
    ]
    .concat()
}

pub fn read_predicate_words_len() -> Vec<asm::Op> {
    [
        read_predicate_len(),
        ops![
            PUSH: predicate_layout_offset::WORDS,
            SUB,
        ],
    ]
    .concat()
}

pub fn read_predicate_words() -> Vec<asm::Op> {
    [
        push_predicate_i(),
        ops![
            PUSH: predicate_layout_offset::WORDS,
        ],
        read_predicate_words_len(),
        ops![DECISION_VAR,],
    ]
    .concat()
}

pub fn read_predicate_size() -> Vec<asm::Op> {
    read_dec_var(dec_var_slot_offset::NUM_PREDICATES, 0, 1)
}

pub fn read_salt() -> Vec<asm::Op> {
    read_dec_var(dec_var_slot_offset::SALT, 0, 4)
}

pub fn match_asm_tag(tag_asm: Vec<asm::Op>, tag: Word, body: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        tag_asm,
        ops![
            PUSH: tag,
            EQ,
            NOT,
            PUSH: (body.len() + 1) as Word,
            SWAP,
            JUMP_FORWARD_IF,
        ],
        body,
    ]
    .concat()
}

pub fn match_tag(tag: Word, body: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        read_predicate_tag(),
        ops![
            PUSH: tag,
            EQ,
            NOT,
            PUSH: (body.len() + 1) as Word,
            SWAP,
            JUMP_FORWARD_IF,
        ],
        body,
    ]
    .concat()
}

pub fn panic_on_no_match_asm_tag(tag: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        tag,
        ops![
            DUP,
            PUSH: tags::NUM_TAGS,
            GTE,
            SWAP,
            PUSH: 0,
            LT,
            OR,
            PANIC_IF,
        ],
    ]
    .concat()
}

pub fn panic_on_no_match() -> Vec<asm::Op> {
    [
        read_predicate_tag(),
        ops![
            DUP,
            PUSH: tags::NUM_TAGS,
            GTE,
            SWAP,
            PUSH: 0,
            LT,
            OR,
            PANIC_IF,
        ],
    ]
    .concat()
}

pub fn read_state_slot(slot_ix: Word, value_ix: Word, len: Word, delta: bool) -> Vec<asm::Op> {
    ops![
        PUSH: slot_ix,
        PUSH: value_ix,
        PUSH: len,
        PUSH: delta as Word,
        STATE,
    ]
}

pub fn state_slot_len(slot_ix: Word, delta: bool) -> Vec<asm::Op> {
    ops![
        PUSH: slot_ix,
        PUSH: delta as Word,
        STATE_LEN,
    ]
}

pub fn state_slot_is_nil(slot_ix: Word, delta: bool) -> Vec<asm::Op> {
    [
        state_slot_len(slot_ix, delta),
        ops![
            PUSH: 0,
            EQ,
        ],
    ]
    .concat()
}

pub fn jump_if(body: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        ops![
            PUSH: (body.len() + 1) as Word,
            SWAP,
            JUMP_FORWARD_IF,
        ],
        body,
    ]
    .concat()
}

pub fn jump_if_cond(cond: Vec<asm::Op>, body: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        cond,
        ops![
            PUSH: (body.len() + 1) as Word,
            SWAP,
            JUMP_FORWARD_IF,
        ],
        body,
    ]
    .concat()
}

pub fn debug() -> Vec<asm::Op> {
    ops![
        PUSH: 1,
        PANIC_IF,
    ]
}

pub fn debug_i(i: Word) -> Vec<asm::Op> {
    ops![
        REPEAT_COUNTER,
        PUSH: i,
        EQ,
        PANIC_IF,
    ]
}
