use super::*;

fn predicate_addrs_i() -> Vec<sasm::Op> {
    state::ops![
        PUSH: PREDICATE_ADDRS_SLOT_IX,
        REPEAT_COUNTER,
        PUSH: 5,
        MUL,
    ]
}

fn predicate_addrs_i_address() -> Vec<sasm::Op> {
    use state::ops;
    state::opsv![
        predicate_addrs_i(),
        ops![
            PUSH: 1,
            ADD,
            PUSH: 4,
            PUSH: 0,
            STATE,
        ]
    ]
}

fn predicate_addrs_i_tag() -> Vec<sasm::Op> {
    use state::ops;
    state::opsv![
        predicate_addrs_i(),
        ops![
            PUSH: 1,
            PUSH: 0,
            STATE,
        ]
    ]
}

fn state_mem_i_not_nil() -> Vec<sasm::Op> {
    state::ops![
        REPEAT_COUNTER,
        VALUE_LEN,
        PUSH: 0,
        EQ,
        NOT,
    ]
}

fn state_mem_i_store_words(num: Word) -> Vec<sasm::Op> {
    state::ops![
        PUSH: num,
        REPEAT_COUNTER,
        STORE,
    ]
}

fn state_mem_i_clear() -> Vec<sasm::Op> {
    state::ops![REPEAT_COUNTER, CLEAR,]
}

fn body() -> Vec<sasm::Op> {
    use state::ops;
    state::opsv![
        ops![
            PUSH: PREDICATE_ADDR_STORAGE_INDEX,
        ],
        predicate_addrs_i_address(),
        alloc(1),
        single_key_at_counter(5),
        predicate_addrs_i_tag(),
        state_mem_i_not_nil(),
        state_mem_i_clear(),
        state_mem_i_store_words(2),
        // ops![
        //     REPEAT_COUNTER,
        //     PUSH: 1,
        //     EQ,
        //     PANIC_IF,
        // ],
        // s_debug(),
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
        s_match_asm_tag(predicate_addrs_i_tag(), NEW_TAG, body()),
        //
        // EXISTING_TAG
        s_match_asm_tag(predicate_addrs_i_tag(), EXISTING_TAG, body()),
        //
        // No match
        s_panic_on_no_match_asm_tag(predicate_addrs_i_tag()),
        // Loop end
        ops![REPEAT_END],
    ]
}
