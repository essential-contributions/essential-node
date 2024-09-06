use super::*;

const CONTRACT_EXISTS_STATE_SLOT: Word = 0;
const PREDICATE_ADDRS_STATE_SLOT: Word = CONTRACT_EXISTS_STATE_SLOT + 1;
const CONTRACT_ADDR_STATE_SLOT: Word = PREDICATE_ADDRS_STATE_SLOT + 1;

fn new_tag_body() -> Vec<sasm::Op> {
    state::opsv![
        s_read_predicate_words(),
        s_read_predicate_words_len(),
        s_read_predicate_padding_len(),
        state::ops![SHA_256]
    ]
}

fn write_tag_and_address() -> Vec<sasm::Op> {
    use state::ops;
    state::opsv![
        s_jump_if_cond(
            ops![
                REPEAT_COUNTER,
                PUSH: 0,
                EQ,
            ],
            load_all_state_slot(PREDICATE_ADDRS_STATE_SLOT),
        ),
        ops![
            REPEAT_COUNTER,
            PUSH: 5,
            MUL,
            PUSH: 4,
            ADD,
            PUSH: PREDICATE_ADDRS_STATE_SLOT,
            STORE,
        ],
        s_read_predicate_tag(),
        load_all_state_slot(PREDICATE_ADDRS_STATE_SLOT),
        ops![
            REPEAT_COUNTER,
            PUSH: 5,
            MUL,
            PUSH: 5,
            ADD,
            PUSH: PREDICATE_ADDRS_STATE_SLOT,
            STORE,
        ],
        load_state_slot(PREDICATE_ADDRS_STATE_SLOT, 1, 4),
    ]
}

pub fn read_contract_addr() -> Vec<u8> {
    use state::ops;
    state::opsi![
        ops![
            PUSH: CONTRACT_ADDR_STORAGE_INDEX,
        ],
        alloc(3),
        // for i in 0..predicates_size
        s_read_predicate_size(),
        ops![
            PUSH: 1,
            REPEAT,
        ],
        // match tag
        // NEW_TAG
        s_match_tag(NEW_TAG, new_tag_body()),
        //
        // EXISTING_TAG
        s_match_tag(EXISTING_TAG, s_read_predicate_words()),
        //
        // No match
        s_panic_on_no_match(),
        // Write tag and predicate address to storage
        write_tag_and_address(),
        //
        // loop end
        ops![REPEAT_END,],
        // salt
        //
        s_read_salt(),
        //
        // sha256([predicate_hashes..., salt])
        s_read_predicate_size(),
        ops![
            PUSH: 4,
            MUL,
            PUSH: 4,
            ADD,
            PUSH: 0,
            SHA_256,
        ],
        // Write contract_addr as set to storage
        store_state_slot(4, CONTRACT_ADDR_STATE_SLOT),
        load_state_slot(CONTRACT_ADDR_STATE_SLOT, 0, 4),
        //
        // state deployed_contract = storage::contracts[hash];
        single_key(5, CONTRACT_EXISTS_STATE_SLOT),
    ]
}
