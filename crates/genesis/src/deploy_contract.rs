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

use crate::utils::{constraint, state};

const CONTRACT_ADDR_STORAGE_INDEX: Word = 0;

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
    let state_read = vec![read_contract_addr()];
    let constraints = vec![];
    Predicate {
        state_read,
        constraints,
        directive: Directive::Satisfy,
    }
}

fn push_predicate_i() -> Vec<sasm::Op> {
    state::ops![
        REPEAT_COUNTER,
        PUSH: PREDICATES_OFFSET,
        ADD,
    ]
}

fn read_dec_var(slot_ix: Word, value_ix: Word, len: Word) -> Vec<sasm::Op> {
    state::ops![
        PUSH: slot_ix,
        PUSH: value_ix,
        PUSH: len,
        DECISION_VAR,
    ]
}

fn read_predicate_tag() -> Vec<sasm::Op> {
    state::opsv![
        push_predicate_i(),
        state::ops![
            PUSH: TAG_VALUE_IX,
            PUSH: 1,
            DECISION_VAR,
        ]
    ]
}

fn read_predicate_padding_len() -> Vec<sasm::Op> {
    state::opsv![
        push_predicate_i(),
        state::ops![
            PUSH: PADDING_LEN_VALUE_IX,
            PUSH: 1,
            DECISION_VAR,
        ]
    ]
}

fn read_predicate_len() -> Vec<sasm::Op> {
    state::opsv![push_predicate_i(), state::ops![DECISION_VAR_LEN,]]
}

fn read_predicate_words_len() -> Vec<sasm::Op> {
    state::opsv![
        read_predicate_len(),
        state::ops![
            PUSH: WORDS_VALUE_IX,
            SUB,
        ]
    ]
}

fn read_predicate_words() -> Vec<sasm::Op> {
    state::opsv![
        push_predicate_i(),
        state::ops![
            PUSH: WORDS_VALUE_IX,
        ],
        read_predicate_words_len(),
        state::ops![DECISION_VAR,]
    ]
}

fn read_predicate_size() -> Vec<sasm::Op> {
    read_dec_var(PREDICATES_SIZE_INDEX, 0, 1)
}

fn read_salt() -> Vec<sasm::Op> {
    read_dec_var(SALT_INDEX, 0, 1)
}

fn match_tag(tag: Word, body: Vec<sasm::Op>) -> Vec<sasm::Op> {
    state::opsv![
        read_predicate_tag(),
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

fn new_tag_body() -> Vec<sasm::Op> {
    state::opsv![
        read_predicate_words(),
        read_predicate_words_len(),
        read_predicate_padding_len(),
        state::ops![SHA_256]
    ]
}

fn read_contract_addr() -> Vec<u8> {
    use state::ops;
    state::opsi![
        // for i in 0..predicates_size
        read_predicate_size(),
        ops![
            PUSH: 1,
            REPEAT
        ],
        // match tag
        // NEW_TAG
        match_tag(NEW_TAG, new_tag_body()),
        //
        // EXISTING_TAG
        match_tag(EXISTING_TAG, read_predicate_words()),
        //
        // loop end
        ops![REPEAT_END,],
        // salt
        //
        read_salt(),
        //
        // sha256([predicate_hashes..., salt])
        read_predicate_size(),
        ops![
            PUSH: 4,
            MUL,
            PUSH: 1,
            ADD,
            PUSH: 0,
            SHA_256,
        ],
    ]
}

fn read_contract_addr2() -> Vec<u8> {
    state::opsc![
        sasm::Stack::Push(PREDICATES_SIZE_INDEX),
        sasm::Stack::Push(0),
        sasm::Stack::Push(1),
        sasm::Access::DecisionVar,
        sasm::Stack::Push(1),
        sasm::Stack::Repeat,
        // Get predicate i
        // Get predicate i slot_ix
        sasm::Access::RepeatCounter,
        sasm::Stack::Push(PREDICATES_OFFSET),
        sasm::Alu::Add,
        sasm::Stack::Dup,
        // Read tag
        sasm::Stack::Push(0),
        sasm::Stack::Push(1),
        sasm::Access::DecisionVar,
        sasm::Stack::Swap,
        sasm::Stack::Dup,
        // Get predicate i slot_ix length
        sasm::Access::DecisionVarLen,
        sasm::Stack::Push(2),
        sasm::Alu::Sub,
        sasm::Stack::Push(2),
        sasm::Stack::Swap,
        sasm::Access::DecisionVar,
        // Read padding len
        sasm::Access::RepeatCounter,
        sasm::Stack::Push(PREDICATES_OFFSET),
        sasm::Alu::Add,
        sasm::Stack::Dup,
        sasm::Stack::Push(1),
        sasm::Stack::Push(1),
        sasm::Access::DecisionVar,
        sasm::Stack::Swap,
        // Read tag
        sasm::Stack::Push(0),
        sasm::Stack::Push(1),
        sasm::Access::DecisionVar,
        // Jump to tag code
        sasm::Stack::Push(NEW_TAG),
        sasm::Pred::Eq,
        sasm::Pred::Not,
        // Dist to jump
        sasm::Stack::Push(14),
        sasm::Stack::Swap,
        sasm::TotalControlFlow::JumpForwardIf,
        // case NEW_TAG
        sasm::Access::RepeatCounter,
        sasm::Stack::Push(PREDICATES_OFFSET),
        sasm::Alu::Add,
        sasm::Access::DecisionVarLen,
        sasm::Stack::Push(2),
        sasm::Alu::Sub,
        sasm::Stack::Swap,
        sasm::Crypto::Sha256,
        // Store tag and hash
        sasm::Stack::Push(1),
        sasm::StateSlots::AllocSlots,
        sasm::Stack::Push(5),
        sasm::Access::RepeatCounter,
        sasm::StateSlots::Store,
        // End case NEW_TAG
        //
        // Read tag
        sasm::Access::RepeatCounter,
        sasm::Stack::Push(PREDICATES_OFFSET),
        sasm::Alu::Add,
        sasm::Stack::Push(0),
        sasm::Stack::Push(1),
        sasm::Access::DecisionVar,
        // Jump to tag code
        sasm::Stack::Push(EXISTING_TAG),
        sasm::Pred::Eq,
        sasm::Pred::Not,
        // Dist to jump
        sasm::Stack::Push(1),
        sasm::Stack::Swap,
        sasm::TotalControlFlow::JumpForwardIf,
        // case EXISTING_TAG
        // End case EXISTING_TAG

        // Read tag
        sasm::Access::RepeatCounter,
        sasm::Stack::Push(PREDICATES_OFFSET),
        sasm::Alu::Add,
        sasm::Stack::Push(0),
        sasm::Stack::Push(1),
        sasm::Access::DecisionVar,
        sasm::Stack::Dup,
        sasm::Stack::Push(NUM_TAGS - 1),
        sasm::Pred::Gt,
        sasm::Stack::Swap,
        sasm::Stack::Push(0),
        sasm::Pred::Lt,
        sasm::Pred::Or,
        sasm::TotalControlFlow::PanicIf,
        sasm::Stack::RepeatEnd,
    ]
}

enum DeployedPredicate<'p> {
    New(&'p Predicate),
    Existing(&'p ContentAddress),
}

fn predicates_to_dec_vars<'p>(
    salt: &Hash,
    predicates: impl IntoIterator<Item = DeployedPredicate<'p>>,
) -> Vec<Value> {
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
                dbg!(v.len());
                dbg!(&v);
                v
            }
            DeployedPredicate::Existing(addr) => {
                let mut v = vec![EXISTING_TAG, 0];
                v.extend(word_4_from_u8_32(addr.0));
                dbg!(v.len());
                dbg!(&v);
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
