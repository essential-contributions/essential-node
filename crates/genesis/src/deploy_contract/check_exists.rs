use super::*;

fn predicate_exists_i_tag() -> Vec<asm::Op> {
    constraint::ops![
        // slot_ix
        PUSH: PREDICATE_EXISTS_SLOT_IX,
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
    constraint::ops![
        // slot_ix
        PUSH: PREDICATE_EXISTS_SLOT_IX,
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
    use constraint::ops;
    constraint::opsv![
        predicate_exists_i_bool(false),
        ops![NOT],
        predicate_exists_i_bool(true),
        ops![AND],
    ]
}

fn existing_tag_body() -> Vec<asm::Op> {
    use constraint::ops;
    constraint::opsv![
        predicate_exists_i_bool(false),
        predicate_exists_i_bool(true),
        ops![AND],
    ]
}

pub fn check_exists() -> Vec<u8> {
    use constraint::ops;
    constraint::opsi![
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
        match_asm_tag(predicate_exists_i_tag(), NEW_TAG, new_tag_body()),
        //
        // EXISTING_TAG
        match_asm_tag(predicate_exists_i_tag(), EXISTING_TAG, existing_tag_body()),
        //
        // No match
        panic_on_no_match_asm_tag(predicate_exists_i_tag()),
        ops![AND,],
        // Loop end
        ops![REPEAT_END],
    ]
}
