use super::*;

fn new_tag_body() -> Vec<asm::Op> {
    constraint::ops![
        REPEAT_COUNTER,
        PUSH: 1,
        ADD,
        PUSH: 0,
        PUSH: 1,
        PUSH: 0,
        STATE,
        PUSH: 0,
        EQ,
        REPEAT_COUNTER,
        PUSH: 1,
        ADD,
        PUSH: 0,
        PUSH: 1,
        PUSH: 1,
        STATE,
        PUSH: 1,
        EQ,
        AND,
    ]
}

fn existing_tag_body() -> Vec<asm::Op> {
    constraint::ops![
        REPEAT_COUNTER,
        PUSH: 1,
        ADD,
        PUSH: 0,
        PUSH: 1,
        PUSH: 0,
        STATE,
        PUSH: 1,
        EQ,
        REPEAT_COUNTER,
        PUSH: 1,
        ADD,
        PUSH: 0,
        PUSH: 1,
        PUSH: 1,
        STATE,
        PUSH: 1,
        EQ,
        AND,
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
        match_tag(NEW_TAG, new_tag_body()),
        //
        // EXISTING_TAG
        match_tag(EXISTING_TAG, existing_tag_body()),
        //
        // No match
        panic_on_no_match(),
        ops![AND,],
        // Loop end
        ops![REPEAT_END],
    ]
}
