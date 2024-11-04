use crate::config::*;

#[derive(Clone, Copy, Debug)]
pub enum SlotType {
    Idle,
    Dedicate,
    Shared,
}

impl From<u8> for SlotType {
    fn from(value: u8) -> Self {
        match value {
            0 => SlotType::Idle,
            1 => SlotType::Dedicate,
            2 => SlotType::Shared,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub slot_type: SlotType,
    pub slot_number: u32,
    pub sender: u32,
    pub receiver: u32,
}

impl Slot {
    pub fn new(slot_type: SlotType, slot_number: u32, sender: u32, receiver: u32) -> Self {
        Self {
            slot_type,
            slot_number,
            sender,
            receiver,
        }
    }
    pub fn new_idle(slot_number: u32) -> Self {
        Self::new(SlotType::Idle, slot_number, 0, 0)
    }

    pub fn new_dedicate(slot_number: u32, sender: u32, receiver: u32) -> Self {
        Self::new(SlotType::Dedicate, slot_number, sender, receiver)
    }

    pub fn new_shared(slot_number: u32, sender: u32, receiver: u32) -> Self {
        Self::new(SlotType::Shared, slot_number, sender, receiver)
    }
}

pub struct Schedule {
    pub slots: [Slot; SLOTFRAME_SIZE as usize],
}

impl Schedule {
    pub fn default() -> Self {
        Self {
            slots: core::array::from_fn(|i| Slot::new_idle(i as u32)),
        }
    }

    pub fn new() -> Self {
        Self {
            slots: [
                Slot::new_dedicate(0, 1, 2),
                Slot::new_dedicate(1, 2, 1),
                Slot::new_dedicate(2, 1, 2),
                Slot::new_dedicate(3, 2, 1),
                Slot::new_shared(4, ANY_NODE, ORCHESTRATOR_ID),
                Slot::new_shared(5, ORCHESTRATOR_ID, ANY_NODE),
                Slot::new_shared(6, ANY_NODE, ORCHESTRATOR_ID),
                Slot::new_shared(7, ORCHESTRATOR_ID, ANY_NODE),
            ],
        }
    }
}
