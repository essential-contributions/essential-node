use super::constraint::*;
use crate::deploy_contract::{dec_var_slot_offset, state_slot_offset};
use essential_constraint_asm as asm;
use essential_types::Word;

#[cfg(test)]
mod tests;

pub fn unique_hashes_bytes() -> Vec<u8> {
    asm::to_bytes(unique_hashes()).collect()
}
/// constraint forall i in 0..num_predicates {
///     forall j in i + 1..num_predicates {
///         hashes[i] != hashes[j]
///     }
/// };
fn unique_hashes() -> Vec<asm::Op> {
    [
        push_num_predicates(),
        vec![
            // Alloc temp for storing outer counter
            PUSH(1),
            ALOCT,
            POP,
            // base case
            PUSH(true as Word),
            PUSH(1),
            DUPF,
        ],
        for_loop(inner_loop(
            [
                // i
                vec![PUSH(0), LOD],
                push_hash(),
                // j
                vec![
                    // i
                    PUSH(0),
                    LOD,
                    // i + 1
                    PUSH(1),
                    ADD,
                    // j
                    REPC,
                    ADD,
                ],
                push_hash(),
                vec![PUSH(4), EQRA, NOT, AND],
            ]
            .concat(),
        )),
        vec![SWAP, POP],
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
    [vec![PUSH(true as Word), REP], body, vec![REPE]].concat()
}

fn inner_loop(body: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        vec![
            // Get num_predicates
            PUSH(1),
            DUPF,
            // i + 1
            REPC,
            PUSH(1),
            ADD,
            SUB, // num_predicates - (i + 1)
            // Store outer counter
            PUSH(0),
            REPC,
            STO,
            // Start loop
            PUSH(true as Word),
            REP,
        ],
        body,
        vec![REPE],
    ]
    .concat()
}
