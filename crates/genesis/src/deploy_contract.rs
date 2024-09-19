use essential_types::{
    contract::Contract,
    convert::{word_4_from_u8_32, word_from_bytes},
    predicate::{header::PredicateError, Predicate},
    ContentAddress, Hash, Value, Word,
};

#[cfg(test)]
mod tests;

mod check_exists;
mod constrain_keys;
mod delta_contract;
mod read_contract_addr;
mod read_predicate_addr;
mod validate_contract;

mod constraint;
mod state;

mod storage_index {
    use essential_types::Word;

    pub const CONTRACTS: Word = 0;
    pub const PREDICATES: Word = CONTRACTS + 1;
}

mod dec_var_slot_offset {
    use essential_types::Word;

    pub const SALT: Word = 0;
    pub const NUM_PREDICATES: Word = SALT + 1;
    pub const PREDICATES: Word = NUM_PREDICATES + 1;
}

mod tags {
    use essential_types::Word;

    pub const NEW: Word = 0;
    pub const EXISTING: Word = NEW + 1;
    pub const NUM_TAGS: Word = EXISTING + 1;
}

mod predicate_layout_offset {
    use essential_types::Word;

    pub const TAG: Word = 0;
    pub const PADDING_LEN: Word = TAG + 1;
    pub const WORDS: Word = PADDING_LEN + 1;
}

mod state_slot_offset {
    use essential_types::Word;

    pub const CONTRACT_EXISTS: Word = 0;
    pub const PREDICATE_ADDRS: Word = CONTRACT_EXISTS + 1;
    pub const CONTRACT_ADDR: Word = PREDICATE_ADDRS + 1;
    pub const PREDICATE_EXISTS: Word = CONTRACT_ADDR + 1;
}

mod state_mem_offset {
    use essential_types::Word;

    pub const CONTRACT_EXISTS: Word = 0;
    pub const PREDICATE_ADDRS: Word = CONTRACT_EXISTS + 1;
    pub const CONTRACT_ADDR: Word = PREDICATE_ADDRS + 1;
}

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
    let constraints = vec![
        delta_contract::delta_contract(),
        check_exists::check_exists(),
        constrain_keys::constrain_keys(),
        validate_contract::validate_contract(),
    ];
    Predicate {
        state_read,
        constraints,
    }
}

pub enum DeployedPredicate<'p> {
    New(&'p Predicate),
    Existing(&'p ContentAddress),
}

pub fn predicates_to_dec_vars<'p>(
    salt: &Hash,
    predicates: impl IntoIterator<Item = DeployedPredicate<'p>>,
) -> Result<Vec<Value>, PredicateError> {
    let mut predicates = predicates.into_iter().collect::<Vec<_>>();
    predicates.sort_by_key(|p| match p {
        DeployedPredicate::New(p) => essential_hash::content_addr(*p),
        DeployedPredicate::Existing(addr) => (*addr).clone(),
    });
    let predicates: Vec<_> = predicates
        .into_iter()
        .map(|p| {
            let v = match p {
                DeployedPredicate::New(p) => {
                    let bytes: Vec<_> = p.encode()?.collect();
                    let padding_len =
                        core::mem::size_of::<Word>() - (bytes.len() % core::mem::size_of::<Word>());
                    let mut v = vec![tags::NEW, padding_len as Word];
                    let iter = bytes.chunks(core::mem::size_of::<Word>()).map(|chunk| {
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
                    let mut v = vec![tags::EXISTING, 0];
                    v.extend(word_4_from_u8_32(addr.0));
                    v
                }
            };
            Ok(v)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = vec![
        word_4_from_u8_32(*salt).to_vec(),
        vec![predicates.len() as Word],
    ];
    out.extend(predicates);
    Ok(out)
}
