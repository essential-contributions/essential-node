use super::*;

fn new_tag_body() -> Vec<sasm::Op> {
    use state::ops;
    state::opsv![
        ops![
            PUSH: PREDICATE_ADDR_STORAGE_INDEX,
        ],
        s_read_predicate_words(),
        s_read_predicate_words_len(),
        s_read_predicate_padding_len(),
        ops![SHA_256,],
        alloc(1),
        single_key_at_counter(5),
        ops![
            REPEAT_COUNTER,
            VALUE_LEN,
            PUSH: 0,
            EQ,
            NOT,
            REPEAT_COUNTER,
            CLEAR,
            PUSH: 1,
            REPEAT_COUNTER,
            STORE,
        ],
    ]
}

fn existing_tag_body() -> Vec<sasm::Op> {
    use state::ops;
    state::opsv![
        ops![
            PUSH: PREDICATE_ADDR_STORAGE_INDEX,
        ],
        s_read_predicate_words(),
        alloc(1),
        single_key_at_counter(5),
        ops![
            REPEAT_COUNTER,
            VALUE_LEN,
            PUSH: 0,
            EQ,
            NOT,
            REPEAT_COUNTER,
            CLEAR,
            PUSH: 1,
            REPEAT_COUNTER,
            STORE,
        ],
    ]
}

pub fn read_predicate_addr() -> Vec<u8> {
    use state::ops;
    state::opsi![
        // for i in 0..predicates_size
        s_read_predicate_size(),
        ops![
            PUSH: 1,
            REPEAT
        ],
        // match tag
        // NEW_TAG
        s_match_tag(NEW_TAG, new_tag_body()),
        //
        // EXISTING_TAG
        s_match_tag(EXISTING_TAG, existing_tag_body()),
        //
        // No match
        s_panic_on_no_match(),
        // Loop end
        ops![REPEAT_END],
    ]
}
