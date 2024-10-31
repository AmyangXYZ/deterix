use crate::config::*;

#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub sender: u32,
    pub receiver: u32,
}

impl Slot {
    pub fn new(sender: u32, receiver: u32) -> Self {
        Self { sender, receiver }
    }
}

pub struct Schedule {
    pub slots: [Slot; SLOTFRAME_SIZE as usize],
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            slots: [
                Slot::new(1, 2),
                Slot::new(2, 1),
                Slot::new(1, 3),
                Slot::new(1, 3),
                Slot::new(ANY_NODE, ORCHESTRATOR_ID),
                Slot::new(ORCHESTRATOR_ID, ANY_NODE),
                Slot::new(ANY_NODE, ORCHESTRATOR_ID),
                Slot::new(ORCHESTRATOR_ID, ANY_NODE),
            ],
        }
    }
}
