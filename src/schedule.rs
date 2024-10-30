use crate::config::*;

pub struct Schedule {
    pub slots: [Slot; SLOTFRAME_SIZE as usize],
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            slots: [
                Slot::new(1, 2),
                Slot::new(2, 1),
                Slot::new(1, 2),
                Slot::new(2, 1),
            ],
        }
    }
}
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
