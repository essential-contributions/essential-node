use crate::deploy_contract::dec_var_slot_offset;
use crate::deploy_contract::predicate_layout_offset;

use super::state::*;
use super::state_slot_offset;
use super::storage_index;
use super::tags;
use essential_state_asm as asm;
use essential_types::Word;

#[cfg(test)]
mod tests;

pub fn find_contract_bytes() -> Vec<u8> {
    asm::to_bytes(find_contract()).collect()
}

pub fn find_contract() -> Vec<asm::Op> {
    [
        vec![PUSH(1), ALOCS], // Allocate the slot for this state
        for_loop(
            [
                predicate_exists(),
                panic_on_no_match(),
            ]
            .concat(),
        ),
        contract_exists(),
    ]
    .concat()
}

fn store_true() -> Vec<asm::Op> {
    vec![
        PUSH(0),            // slot_ix
        REPC,               // value_ix
        PUSH(true as Word), // data
        PUSH(1),            // len
        STOS,
    ]
}

fn predicate_exists() -> Vec<asm::Op> {
    [
        // Storage location
        vec![
            PUSH(0), // slot_ix
            PUSH(0), // value_ix
        ],
        load_all_mem(),
        vec![
            // key begin
            PUSH(storage_index::PREDICATES),
            PUSH(state_slot_offset::GENERATE_HASHES), // slot_ix
            // value_ix
            REPC,
            PUSH(4),
            MUL,
            PUSH(4), // len
            PUSH(0), // delta
            STATE,
            // end of key
            PUSH(5), // key len
            PUSH(1), // num to read
            PUSH(0), // slot_ix
            KRNG,    // Read if predicate exists
            // is storage value len 1
            PUSH(0), // slot_ix
            SMVLEN,
            PUSH(1), // len
            EQ,
        ],
        jump_if_cond(
            vec![DUP, NOT],
            vec![
                // is storage value 1
                PUSH(0), // slot_ix
                PUSH(0), // value_ix
                PUSH(1), // len
                LODS,
                PUSH(1),
                EQ,
                AND,
            ],
        ),
        vec![
            // Data len
            REPC,
            PUSH(1),
            ADD,
            STOS,
        ],
    ]
    .concat()
}

fn contract_exists() -> Vec<asm::Op> {
    [
        // Storage location
        vec![
            PUSH(0), // slot_ix
            PUSH(0), // value_ix
        ],
        load_all_mem(),
        vec![
            // key begin
            PUSH(storage_index::CONTRACTS),
            PUSH(state_slot_offset::GENERATE_HASHES), // slot_ix
        ],
        push_num_predicates(), // value_ix
        vec![
            PUSH(4),
            MUL,
            PUSH(4), // len
            PUSH(0), // delta
            STATE,
            // end of key
            PUSH(5), // key len
            PUSH(1), // num to read
            PUSH(0), // slot_ix
            KRNG,    // Read if contract exists
            // is storage value len 1
            PUSH(0), // slot_ix
            SMVLEN,
            PUSH(1), // len
            EQ,
        ],
        jump_if_cond(
            vec![DUP, NOT],
            vec![
                // is storage value 1
                PUSH(0), // slot_ix
                PUSH(0), // value_ix
                PUSH(1), // len
                LODS,
                PUSH(1),
                EQ,
                AND,
            ],
        ),
        // data len
        push_num_predicates(),
        vec![PUSH(1), ADD, STOS],
    ]
    .concat()
}

fn load_all_mem() -> Vec<asm::Op> {
    vec![
        PUSH(0), // slot_ix
        DUP,     // [slot_ix, slot_ix]
        SMVLEN,  // [slot_ix, len]
        PUSH(0), // value_ix
        SWAP,    // [slot_ix, value_ix, len]
        LODS,
    ]
}

fn contract_storage_location() -> Vec<asm::Op> {
    [
        vec![
            PUSH(0), // slot_ix
        ],
        push_num_predicates(), // value_ix
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

fn for_loop(body: Vec<asm::Op>) -> Vec<asm::Op> {
    [
        push_num_predicates(),
        vec![PUSH(true as Word), REP],
        body,
        vec![REPE],
    ]
    .concat()
}
