pub mod constraint {
    // macro_rules! ops {
    //     ($($op:expr),* $(,)?) => {
    //         essential_constraint_asm::to_bytes(vec![$(essential_constraint_asm::Op::from($op)),*])
    //             .collect::<Vec<u8>>()
    //     };
    // }

    // pub(crate) use ops;
}

pub mod state {
    macro_rules! opsc {
        ($($op:expr),* $(,)?) => {
            essential_state_asm::to_bytes(vec![$(essential_state_asm::Op::from($op)),*])
                .collect::<Vec<u8>>()
        };
    }

    pub(crate) use opsc;

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
        (JUMP_FORWARD_IF) => {
            $crate::sasm::Op::from($crate::sasm::TotalControlFlow::JumpForwardIf)
        };
        (SHA_256) => {
            $crate::sasm::Op::from($crate::sasm::Crypto::Sha256)
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
