use super::*;

fn new_tag_body() -> Vec<sasm::Op> {
    state::opsv![
        s_read_predicate_words(),
        s_read_predicate_words_len(),
        s_read_predicate_padding_len(),
        state::ops![SHA_256]
    ]
}

pub fn read_contract_addr() -> Vec<u8> {
    use state::ops;
    state::opsi![
        ops![
            PUSH: CONTRACT_ADDR_STORAGE_INDEX,
        ],
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
        //
        // state deployed_contract = storage::contracts[hash];
        alloc(1),
        single_key(5, 0),
    ]
}
