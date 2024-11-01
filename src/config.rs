pub const HEADER_SIZE: usize = 176;
pub const DATA_SIZE: usize = 1024;
pub const PACKET_SIZE: usize = HEADER_SIZE + DATA_SIZE + 2; // 2 bytes for data size

pub const MAGIC: u16 = 0xCAFE;

pub const SLOT_DURATION: u64 = 500; // in microseconds
pub const CLEAR_WINDOW: u64 = SLOT_DURATION / 10; // in microseconds
pub const TX_WINDOW: u64 = SLOT_DURATION / 2; // in microseconds
pub const ACK_WINDOW: u64 = SLOT_DURATION * 3 / 10; // in microseconds
pub const GUARD_BAND: u64 = SLOT_DURATION / 10; // in microseconds

pub const SLOTFRAME_SIZE: usize = 8; // in slots

pub const ORCHESTRATOR_ID: u32 = 0;
pub const ANY_NODE: u32 = u32::MAX;

pub const PACKET_POOL_SIZE: usize = 512;

#[cfg(target_os = "linux")]
pub const THREAD_PRIORITY_SLOT_TICKER: i32 = 90;
#[cfg(target_os = "linux")]
pub const THREAD_PRIORITY_PACKET_QUEUE: i32 = 90;

pub const CORE_AFFINITY_SLOT_TICKER: usize = 1;
pub const CORE_AFFINITY_PACKET_QUEUE: usize = 0;
