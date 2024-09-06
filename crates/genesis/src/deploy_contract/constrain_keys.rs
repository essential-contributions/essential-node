use super::*;

fn predicate_addrs_i() -> Vec<asm::Op> {
    constraint::ops![
        PUSH: PREDICATE_ADDRS_SLOT_IX,
        REPEAT_COUNTER,
        PUSH: 5,
        MUL,
    ]
}

fn predicate_addrs_i_address() -> Vec<asm::Op> {
    use constraint::ops;
    constraint::opsv![
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

fn predicate_addrs_i_tag() -> Vec<asm::Op> {
    use constraint::ops;
    constraint::opsv![
        predicate_addrs_i(),
        ops![
            PUSH: 1,
            PUSH: 0,
            STATE,
        ]
    ]
}

fn body() -> Vec<asm::Op> {
    use constraint::ops;
    constraint::opsv![
        ops![PUSH: PREDICATE_ADDR_STORAGE_INDEX],
        predicate_addrs_i_address(),
        ops![
            PUSH: 5,
            PUSH: 0,
            TEMP_LOAD,
            PUSH: 1,
            ADD,
            PUSH: 0,
            SWAP,
            TEMP_STORE,
        ],
    ]
}

pub fn constrain_keys() -> Vec<u8> {
    use constraint::ops;
    constraint::opsi![
        ops![
            PUSH: 1,
            TEMP_ALLOC,
            POP,
        ],
        // for i in 0..predicates_size
        read_predicate_size(),
        ops![
            PUSH: 1,
            REPEAT
        ],
        // match tag
        // NEW_TAG
        match_asm_tag(predicate_addrs_i_tag(), NEW_TAG, body()),
        //
        // No match
        panic_on_no_match_asm_tag(predicate_addrs_i_tag()),
        // Loop end
        ops![
            REPEAT_END,
            PUSH: CONTRACT_ADDR_STORAGE_INDEX,
        ],
        read_state_slot(CONTRACT_ADDR_SLOT_IX, 0, 4, false),
        ops![
            PUSH: 5,
            PUSH: 0,
            TEMP_LOAD,
            PUSH: 1,
            ADD,
            PUSH: 6,
            MUL,
        ],
        ops![MUT_KEYS, EQ_SET],
    ]
}
