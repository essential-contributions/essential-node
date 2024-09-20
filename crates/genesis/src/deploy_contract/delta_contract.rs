use super::constraint::*;
use super::state_slot_offset;
use essential_constraint_asm as asm;

pub fn delta_contract() -> Vec<u8> {
    let r = [
        state_slot_is_nil(state_slot_offset::CONTRACT_EXISTS, false),
        state_slot_is_nil(state_slot_offset::CONTRACT_EXISTS, true),
        jump_if(
            [
                read_state_slot(state_slot_offset::CONTRACT_EXISTS, 0, 1, true),
                vec![PUSH(1), EQ, AND],
            ]
            .concat(),
        ),
        state_slot_len(state_slot_offset::CONTRACT_EXISTS, true),
        vec![PUSH(1), EQ, AND],
    ]
    .concat();
    asm::to_bytes(r).collect()
}
