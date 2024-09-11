use super::constraint::*;
use super::state_slot_offset;
use super::tags;
use crate::utils::constraint::*;
use essential_constraint_asm as asm;
use essential_types::Word;

fn predicate_exists_i_tag() -> Vec<asm::Op> {
    ops![
        // slot_ix
        PUSH: state_slot_offset::PREDICATE_EXISTS,
        REPEAT_COUNTER,
        ADD,
        // value_ix
        PUSH: 0,
        // len
        PUSH: 1,
        // delta
        PUSH: 0,
        STATE,
    ]
}

fn predicate_exists_i_bool(delta: bool) -> Vec<asm::Op> {
    ops![
        // slot_ix
        PUSH: state_slot_offset::PREDICATE_EXISTS,
        REPEAT_COUNTER,
        ADD,
        // value_ix
        PUSH: 1,
        // len
        PUSH: 1,
        // delta
        PUSH: delta as Word,
        STATE,
    ]
}

fn new_tag_body() -> Vec<asm::Op> {
    [
        predicate_exists_i_bool(false),
        ops![NOT],
        predicate_exists_i_bool(true),
        ops![AND],
    ]
    .concat()
}

fn existing_tag_body() -> Vec<asm::Op> {
    [
        predicate_exists_i_bool(false),
        predicate_exists_i_bool(true),
        ops![AND],
    ]
    .concat()
}

pub fn check_exists() -> Vec<u8> {
    let r = [
        ops![
            PUSH: 1,
        ],
        // for i in 0..predicates_size
        read_predicate_size(),
        ops![
            PUSH: 1,
            REPEAT
        ],
        // match tag
        // NEW_TAG
        match_asm_tag(predicate_exists_i_tag(), tags::NEW, new_tag_body()),
        //
        // EXISTING_TAG
        match_asm_tag(
            predicate_exists_i_tag(),
            tags::EXISTING,
            existing_tag_body(),
        ),
        //
        // No match
        panic_on_no_match_asm_tag(predicate_exists_i_tag()),
        ops![AND,],
        // Loop end
        ops![REPEAT_END],
    ]
    .concat();
    asm::to_bytes(r).collect()
}
