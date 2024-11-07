pub const HEADER_SIZE: usize = 176;
pub const DATA_SIZE: usize = 1024;
pub const PACKET_SIZE: usize = HEADER_SIZE + DATA_SIZE + 2; // 2 bytes for data size

pub const MAX_RETRIES: usize = 3;

pub const MAGIC: u16 = 0xCAFE;

pub const SLOTFRAME_SIZE: usize = 10; // in slots

pub const ORCHESTRATOR_ID: u32 = 0;
pub const ANY_NODE: u32 = u32::MAX;

pub const PACKET_POOL_SIZE: usize = 512;

#[cfg(target_os = "linux")]
pub const THREAD_PRIORITY_SLOT_TICKER: i32 = 99;
#[cfg(target_os = "linux")]
pub const THREAD_PRIORITY_PACKET_QUEUE: i32 = 99;
