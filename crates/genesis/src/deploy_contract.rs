use essential_constraint_asm as asm;
use essential_state_asm as sasm;
use essential_types::{
    contract::Contract,
    convert::{word_4_from_u8_32, word_from_bytes},
    predicate::{Directive, Predicate},
    ContentAddress, Hash, Value, Word,
};

#[cfg(test)]
mod tests;

mod check_exists;
mod read_contract_addr;
mod read_predicate_addr;

use crate::utils::{constraint, state};

const CONTRACT_ADDR_STORAGE_INDEX: Word = 0;
const PREDICATE_ADDR_STORAGE_INDEX: Word = CONTRACT_ADDR_STORAGE_INDEX + 1;

const SALT_INDEX: Word = 0;
const PREDICATES_SIZE_INDEX: Word = 1;
const PREDICATES_OFFSET: Word = PREDICATES_SIZE_INDEX + 1;

const NEW_TAG: Word = 0;
const EXISTING_TAG: Word = 1;
const NUM_TAGS: Word = 2;

const TAG_VALUE_IX: Word = 0;
const PADDING_LEN_VALUE_IX: Word = TAG_VALUE_IX + 1;
const WORDS_VALUE_IX: Word = PADDING_LEN_VALUE_IX + 1;

pub fn create() -> Contract {
    let salt = essential_hash::hash(&"deploy_contract");

    let predicates = vec![deploy()];
    Contract { predicates, salt }
}

fn deploy() -> Predicate {
    let state_read = vec![
        read_contract_addr::read_contract_addr(),
        read_predicate_addr::read_predicate_addr(),
    ];
    let constraints = vec![delta_contract(), check_exists::check_exists()];
    Predicate {
        state_read,
        constraints,
        directive: Directive::Satisfy,
    }
}

pub enum DeployedPredicate<'p> {
    New(&'p Predicate),
    Existing(&'p ContentAddress),
}

fn s_push_predicate_i() -> Vec<sasm::Op> {
    to_state(push_predicate_i())
}

fn push_predicate_i() -> Vec<asm::Op> {
    constraint::ops![
        REPEAT_COUNTER,
        PUSH: PREDICATES_OFFSET,
        ADD,
    ]
}

fn to_state(ops: Vec<asm::Op>) -> Vec<sasm::Op> {
    ops.into_iter().map(sasm::Op::from).collect()
}

fn s_read_dec_var(slot_ix: Word, value_ix: Word, len: Word) -> Vec<sasm::Op> {
    to_state(read_dec_var(slot_ix, value_ix, len))
}

fn read_dec_var(slot_ix: Word, value_ix: Word, len: Word) -> Vec<asm::Op> {
    constraint::ops![
        PUSH: slot_ix,
        PUSH: value_ix,
        PUSH: len,
        DECISION_VAR,
    ]
}

fn s_read_predicate_tag() -> Vec<sasm::Op> {
    to_state(read_predicate_tag())
}

fn read_predicate_tag() -> Vec<asm::Op> {
    constraint::opsv![
        push_predicate_i(),
        constraint::ops![
            PUSH: TAG_VALUE_IX,
            PUSH: 1,
            DECISION_VAR,
        ]
    ]
}

fn s_read_predicate_padding_len() -> Vec<sasm::Op> {
    to_state(read_predicate_padding_len())
}

fn read_predicate_padding_len() -> Vec<asm::Op> {
    constraint::opsv![
        push_predicate_i(),
        constraint::ops![
            PUSH: PADDING_LEN_VALUE_IX,
            PUSH: 1,
            DECISION_VAR,
        ]
    ]
}

fn s_read_predicate_len() -> Vec<sasm::Op> {
    to_state(read_predicate_len())
}
fn read_predicate_len() -> Vec<asm::Op> {
    constraint::opsv![push_predicate_i(), constraint::ops![DECISION_VAR_LEN,]]
}

fn s_read_predicate_words_len() -> Vec<sasm::Op> {
    to_state(read_predicate_words_len())
}

fn read_predicate_words_len() -> Vec<asm::Op> {
    constraint::opsv![
        read_predicate_len(),
        constraint::ops![
            PUSH: WORDS_VALUE_IX,
            SUB,
        ]
    ]
}

fn s_read_predicate_words() -> Vec<sasm::Op> {
    to_state(read_predicate_words())
}

fn read_predicate_words() -> Vec<asm::Op> {
    constraint::opsv![
        push_predicate_i(),
        constraint::ops![
            PUSH: WORDS_VALUE_IX,
        ],
        read_predicate_words_len(),
        constraint::ops![DECISION_VAR,]
    ]
}

fn s_read_predicate_size() -> Vec<sasm::Op> {
    to_state(read_predicate_size())
}

fn read_predicate_size() -> Vec<asm::Op> {
    read_dec_var(PREDICATES_SIZE_INDEX, 0, 1)
}

fn s_read_salt() -> Vec<sasm::Op> {
    to_state(read_salt())
}

fn read_salt() -> Vec<asm::Op> {
    read_dec_var(SALT_INDEX, 0, 4)
}

fn alloc(amount: Word) -> Vec<sasm::Op> {
    state::ops![
        PUSH: amount,
        ALLOC_SLOTS,
    ]
}

fn single_key(key_len: Word, slot_ix: Word) -> Vec<sasm::Op> {
    state::ops![
        PUSH: key_len,
        PUSH: 1,
        PUSH: slot_ix,
        KEY_RANGE,
    ]
}

fn single_key_at_counter(key_len: Word) -> Vec<sasm::Op> {
    state::ops![
        PUSH: key_len,
        PUSH: 1,
        REPEAT_COUNTER,
        KEY_RANGE,
    ]
}

fn slot_size(slot_ix: Word) -> Vec<sasm::Op> {
    state::ops![
        PUSH: slot_ix,
        VALUE_LEN,
    ]
}

fn clear_slot(slot_ix: Word) -> Vec<sasm::Op> {
    state::ops![
        PUSH: slot_ix,
        CLEAR,
    ]
}

fn s_match_tag(tag: Word, body: Vec<sasm::Op>) -> Vec<sasm::Op> {
    state::opsv![
        s_read_predicate_tag(),
        state::ops![
            PUSH: tag,
            EQ,
            NOT,
            PUSH: (body.len() + 1) as Word,
            SWAP,
            JUMP_FORWARD_IF,
        ],
        body,
    ]
}

fn match_tag(tag: Word, body: Vec<asm::Op>) -> Vec<asm::Op> {
    constraint::opsv![
        read_predicate_tag(),
        constraint::ops![
            PUSH: tag,
            EQ,
            NOT,
            PUSH: (body.len() + 1) as Word,
            SWAP,
            JUMP_FORWARD_IF,
        ],
        body,
    ]
}

fn s_panic_on_no_match() -> Vec<sasm::Op> {
    to_state(panic_on_no_match())
}
fn panic_on_no_match() -> Vec<asm::Op> {
    constraint::opsv![
        read_predicate_tag(),
        constraint::ops![
            DUP,
            PUSH: NUM_TAGS,
            GTE,
            SWAP,
            PUSH: 0,
            LT,
            OR,
            PANIC_IF,
        ],
    ]
}

fn read_state_slot(slot_ix: Word, value_ix: Word, len: Word, delta: bool) -> Vec<asm::Op> {
    constraint::ops![
        PUSH: slot_ix,
        PUSH: value_ix,
        PUSH: len,
        PUSH: delta as Word,
        STATE,
    ]
}

fn state_slot_len(slot_ix: Word, delta: bool) -> Vec<asm::Op> {
    constraint::ops![
        PUSH: slot_ix,
        PUSH: delta as Word,
        STATE_LEN,
    ]
}

fn state_slot_is_nil(slot_ix: Word, delta: bool) -> Vec<asm::Op> {
    constraint::opsv![
        state_slot_len(slot_ix, delta),
        constraint::ops![
            PUSH: 0,
            EQ,
        ]
    ]
}

fn jump_if(body: Vec<asm::Op>) -> Vec<asm::Op> {
    constraint::opsv![
        constraint::ops![
            PUSH: (body.len() + 1) as Word,
            SWAP,
            JUMP_FORWARD_IF,
        ],
        body,
    ]
}

fn delta_contract() -> Vec<u8> {
    use constraint::ops;
    constraint::opsi![
        state_slot_is_nil(CONTRACT_ADDR_STORAGE_INDEX, false),
        state_slot_is_nil(CONTRACT_ADDR_STORAGE_INDEX, true),
        jump_if(constraint::opsv![
            read_state_slot(CONTRACT_ADDR_STORAGE_INDEX, 0, 1, true),
            ops![
                PUSH: 1,
                EQ,
                AND,
            ]
        ]),
        state_slot_len(CONTRACT_ADDR_STORAGE_INDEX, true),
        ops![
            PUSH: 1,
            EQ,
            AND,
        ]
    ]
}

pub fn predicates_to_dec_vars<'p>(
    salt: &Hash,
    predicates: impl IntoIterator<Item = DeployedPredicate<'p>>,
) -> Vec<Value> {
    let mut predicates = predicates.into_iter().collect::<Vec<_>>();
    predicates.sort_by_key(|p| match p {
        DeployedPredicate::New(p) => essential_hash::content_addr(*p),
        DeployedPredicate::Existing(addr) => (*addr).clone(),
    });
    let predicates: Vec<_> = predicates
        .into_iter()
        .map(|p| match p {
            DeployedPredicate::New(p) => {
                let serialized = essential_hash::serialize(p);
                let padding_len = core::mem::size_of::<Word>()
                    - (serialized.len() % core::mem::size_of::<Word>());
                let mut v = vec![NEW_TAG, padding_len as Word];
                let iter = serialized
                    .chunks(core::mem::size_of::<Word>())
                    .map(|chunk| {
                        if chunk.len() == core::mem::size_of::<Word>() {
                            word_from_bytes(chunk.try_into().unwrap())
                        } else {
                            let mut word = [0u8; core::mem::size_of::<Word>()];
                            word[..chunk.len()].copy_from_slice(chunk);
                            word_from_bytes(word)
                        }
                    });
                v.extend(iter);
                v
            }
            DeployedPredicate::Existing(addr) => {
                let mut v = vec![EXISTING_TAG, 0];
                v.extend(word_4_from_u8_32(addr.0));
                v
            }
        })
        .collect();
    let mut out = vec![
        word_4_from_u8_32(*salt).to_vec(),
        vec![predicates.len() as Word],
    ];
    out.extend(predicates);
    out
}
