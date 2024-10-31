pub const HEADER_SIZE: usize = 176;
pub const DATA_SIZE: usize = 1024;
pub const PACKET_SIZE: usize = HEADER_SIZE + DATA_SIZE + 2; // 2 bytes for data size

pub const MAGIC: u16 = 0xCAFE;
pub const TOKEN: u64 = 0xC0FFEE11;

pub const SLOT_DURATION: u64 = 100_000; // in microseconds
pub const ACK_TIMEOUT: u64 = 100; // in microseconds
pub const SLOTFRAME_SIZE: usize = 4;

pub const PACKET_POOL_SIZE: usize = 512;

#[cfg(target_os = "linux")]
pub const THREAD_PRIORITY_SLOT_TICKER: i32 = 50;
#[cfg(target_os = "linux")]
pub const THREAD_PRIORITY_PACKET_QUEUE: i32 = 50;

pub const CORE_AFFINITY_SLOT_TICKER: usize = 1;
pub const CORE_AFFINITY_PACKET_QUEUE: usize = 0;
