use crate::deploy_contract::dec_var_slot_offset;

use super::constraint::*;
use super::state_slot_offset;
use super::tags;
use essential_constraint_asm as asm;
use essential_types::Word;

#[cfg(test)]
mod tests;

pub fn found_predicates_bytes() -> Vec<u8> {
    asm::to_bytes(found_predicates()).collect()
}

enum Delta {
    Pre = 0,
    Post = 1,
}

/// constraint forall i in 0..num_predicates {
///     match predicates[i] {
///         New(bytes) => !found_predicates[i] && found_predicates[i]',
///         Existing(hash) => found_predicates[i] && found_predicates[i]',
///     }
/// };
fn found_predicates() -> Vec<asm::Op> {
    [
        vec![PUSH(true as Word)], // base case
        for_loop(
            [
                match_tag(tags::NEW, new_body()),
                match_tag(tags::EXISTING, existing_body()),
                panic_on_no_match(),
            ]
            .concat(),
        ),
    ]
    .concat()
}

fn new_body() -> Vec<essential_constraint_asm::Op> {
    [
        push_found_predicate(Delta::Pre),
        vec![NOT],
        push_found_predicate(Delta::Post),
        vec![AND, AND],
    ]
    .concat()
}

fn existing_body() -> Vec<essential_constraint_asm::Op> {
    [
        push_found_predicate(Delta::Pre),
        push_found_predicate(Delta::Post),
        vec![AND, AND],
    ]
    .concat()
}

fn push_found_predicate(delta: Delta) -> Vec<asm::Op> {
    vec![
        PUSH(state_slot_offset::FIND_CONTRACT), // slot_ix
        REPC,                                   // value_ix
        PUSH(1),                                // len
        PUSH(delta as Word),                    // delta
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

fn push_num_predicates() -> Vec<asm::Op> {
    vec![
        PUSH(dec_var_slot_offset::NUM_PREDICATES), // slot_ix
        PUSH(0),                                   // value_ix
        PUSH(1),                                   // len
        VAR,
    ]
}
