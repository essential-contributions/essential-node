pub mod constraint {
    macro_rules! opsi {
        ($($op:expr),* $(,)?) => {{
            let mut v = vec![];
            $(v.extend($op);)*
            $crate::asm::to_bytes(v).collect::<Vec<u8>>()
        }};
    }

    pub(crate) use opsi;

    macro_rules! opsv {
        ($($op:expr),* $(,)?) => {{
            let mut v = vec![];
            $(v.extend($op);)*
            v
        }};
    }

    pub(crate) use opsv;

    macro_rules! op {
        (PUSH: $arg:expr) => {
            $crate::asm::Op::from($crate::asm::Stack::Push($arg))
        };
        (POP) => {
            $crate::asm::Op::from($crate::asm::Stack::Pop)
        };
        (SWAP) => {
            $crate::asm::Op::from($crate::asm::Stack::Swap)
        };
        (DUP) => {
            $crate::asm::Op::from($crate::asm::Stack::Dup)
        };
        (REPEAT) => {
            $crate::asm::Op::from($crate::asm::Stack::Repeat)
        };
        (REPEAT_END) => {
            $crate::asm::Op::from($crate::asm::Stack::RepeatEnd)
        };
        (DECISION_VAR) => {
            $crate::asm::Op::from($crate::asm::Access::DecisionVar)
        };
        (DECISION_VAR_LEN) => {
            $crate::asm::Op::from($crate::asm::Access::DecisionVarLen)
        };
        (REPEAT_COUNTER) => {
            $crate::asm::Op::from($crate::asm::Access::RepeatCounter)
        };
        (STATE) => {
            $crate::asm::Op::from($crate::asm::Access::State)
        };
        (STATE_LEN) => {
            $crate::asm::Op::from($crate::asm::Access::StateLen)
        };
        (MUT_KEYS) => {
            $crate::asm::Op::from($crate::asm::Access::MutKeys)
        };
        (ADD) => {
            $crate::asm::Op::from($crate::asm::Alu::Add)
        };
        (SUB) => {
            $crate::asm::Op::from($crate::asm::Alu::Sub)
        };
        (MUL) => {
            $crate::asm::Op::from($crate::asm::Alu::Mul)
        };
        (EQ) => {
            $crate::asm::Op::from($crate::asm::Pred::Eq)
        };
        (NOT) => {
            $crate::asm::Op::from($crate::asm::Pred::Not)
        };
        (GT) => {
            $crate::asm::Op::from($crate::asm::Pred::Gt)
        };
        (GTE) => {
            $crate::asm::Op::from($crate::asm::Pred::Gte)
        };
        (LT) => {
            $crate::asm::Op::from($crate::asm::Pred::Lt)
        };
        (OR) => {
            $crate::asm::Op::from($crate::asm::Pred::Or)
        };
        (AND) => {
            $crate::asm::Op::from($crate::asm::Pred::And)
        };
        (EQ_SET) => {
            $crate::asm::Op::from($crate::asm::Pred::EqSet)
        };
        (JUMP_FORWARD_IF) => {
            $crate::asm::Op::from($crate::asm::TotalControlFlow::JumpForwardIf)
        };
        (PANIC_IF) => {
            $crate::asm::Op::from($crate::asm::TotalControlFlow::PanicIf)
        };
        (SHA_256) => {
            $crate::asm::Op::from($crate::asm::Crypto::Sha256)
        };
        (TEMP_ALLOC) => {
            $crate::asm::Op::from($crate::asm::Temporary::Alloc)
        };
        (TEMP_LOAD) => {
            $crate::asm::Op::from($crate::asm::Temporary::Load)
        };
        (TEMP_STORE) => {
            $crate::asm::Op::from($crate::asm::Temporary::Store)
        };
    }

    pub(crate) use op;
    //
    macro_rules! ops {
        ($($op:ident $(: $arg:expr)?),* $(,)?) => {
            vec![
                $(
                    constraint::op!($op $( : $arg)?)
                ),*
            ]
        };
    }
    pub(crate) use ops;
}

pub mod state {
    macro_rules! opsi {
        ($($op:expr),* $(,)?) => {{
            let mut v = vec![];
            $(v.extend($op);)*
            $crate::sasm::to_bytes(v).collect::<Vec<u8>>()
        }};
    }

    pub(crate) use opsi;

    macro_rules! opsv {
        ($($op:expr),* $(,)?) => {{
            let mut v = vec![];
            $(v.extend($op);)*
            v
        }};
    }

    pub(crate) use opsv;

    macro_rules! op {
        (PUSH: $arg:expr) => {
            $crate::sasm::Op::from($crate::sasm::Stack::Push($arg))
        };
        (POP) => {
            $crate::sasm::Op::from($crate::sasm::Stack::Pop)
        };
        (SWAP) => {
            $crate::sasm::Op::from($crate::sasm::Stack::Swap)
        };
        (DUP) => {
            $crate::sasm::Op::from($crate::sasm::Stack::Dup)
        };
        (REPEAT) => {
            $crate::sasm::Op::from($crate::sasm::Stack::Repeat)
        };
        (REPEAT_END) => {
            $crate::sasm::Op::from($crate::sasm::Stack::RepeatEnd)
        };
        (DECISION_VAR) => {
            $crate::sasm::Op::from($crate::sasm::Access::DecisionVar)
        };
        (DECISION_VAR_LEN) => {
            $crate::sasm::Op::from($crate::sasm::Access::DecisionVarLen)
        };
        (REPEAT_COUNTER) => {
            $crate::sasm::Op::from($crate::sasm::Access::RepeatCounter)
        };
        (STATE) => {
            $crate::sasm::Op::from($crate::sasm::Access::State)
        };
        (ADD) => {
            $crate::sasm::Op::from($crate::sasm::Alu::Add)
        };
        (SUB) => {
            $crate::sasm::Op::from($crate::sasm::Alu::Sub)
        };
        (MUL) => {
            $crate::sasm::Op::from($crate::sasm::Alu::Mul)
        };
        (EQ) => {
            $crate::sasm::Op::from($crate::sasm::Pred::Eq)
        };
        (NOT) => {
            $crate::sasm::Op::from($crate::sasm::Pred::Not)
        };
        (GT) => {
            $crate::sasm::Op::from($crate::sasm::Pred::Gt)
        };
        (GTE) => {
            $crate::sasm::Op::from($crate::sasm::Pred::Gte)
        };
        (LT) => {
            $crate::sasm::Op::from($crate::sasm::Pred::Lt)
        };
        (OR) => {
            $crate::sasm::Op::from($crate::sasm::Pred::Or)
        };
        (JUMP_FORWARD_IF) => {
            $crate::sasm::Op::from($crate::sasm::TotalControlFlow::JumpForwardIf)
        };
        (PANIC_IF) => {
            $crate::sasm::Op::from($crate::sasm::TotalControlFlow::PanicIf)
        };
        (SHA_256) => {
            $crate::sasm::Op::from($crate::sasm::Crypto::Sha256)
        };
        (ALLOC_SLOTS) => {
            $crate::sasm::Op::from($crate::sasm::StateSlots::AllocSlots)
        };
        (STORE) => {
            $crate::sasm::Op::from($crate::sasm::StateSlots::Store)
        };
        (LOAD) => {
            $crate::sasm::Op::from($crate::sasm::StateSlots::Load)
        };
        (LENGTH) => {
            $crate::sasm::Op::from($crate::sasm::StateSlots::Length)
        };
        (VALUE_LEN) => {
            $crate::sasm::Op::from($crate::sasm::StateSlots::ValueLen)
        };
        (CLEAR) => {
            $crate::sasm::Op::from($crate::sasm::StateSlots::Clear)
        };
        (KEY_RANGE) => {
            $crate::sasm::Op::KeyRange
        };
    }

    pub(crate) use op;
    //
    macro_rules! ops {
        ($($op:ident $(: $arg:expr)?),* $(,)?) => {
            vec![
                $(
                    state::op!($op $( : $arg)?)
                ),*
            ]
        };
    }
    pub(crate) use ops;
}
