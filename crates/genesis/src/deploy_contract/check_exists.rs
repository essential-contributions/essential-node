use super::constraint::*;
use super::state_slot_offset;
use super::tags;
use essential_constraint_asm as asm;
use essential_types::Word;

fn predicate_exists_i_tag() -> Vec<asm::Op> {
    vec![
        // slot_ix
        PUSH(state_slot_offset::PREDICATE_EXISTS),
        REPC,
        ADD,
        // value_ix
        PUSH(0),
        // len
        PUSH(1),
        // delta
        PUSH(0),
        STATE,
    ]
}

fn predicate_exists_i_bool(delta: bool) -> Vec<asm::Op> {
    vec![
        // slot_ix
        PUSH(state_slot_offset::PREDICATE_EXISTS),
        REPC,
        ADD,
        // value_ix
        PUSH(1),
        // len
        PUSH(1),
        // delta
        PUSH(delta as Word),
        STATE,
    ]
}

fn new_tag_body() -> Vec<asm::Op> {
    [
        predicate_exists_i_bool(false),
        vec![NOT],
        predicate_exists_i_bool(true),
        vec![AND],
    ]
    .concat()
}

fn existing_tag_body() -> Vec<asm::Op> {
    [
        predicate_exists_i_bool(false),
        predicate_exists_i_bool(true),
        vec![AND],
    ]
    .concat()
}

pub fn check_exists() -> Vec<u8> {
    let r = [
        vec![PUSH(1)],
        // for i in 0..predicates_size
        read_predicate_size(),
        vec![PUSH(1), REP],
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
        vec![AND],
        // Loop end
        vec![REPE],
    ]
    .concat();
    asm::to_bytes(r).collect()
}
