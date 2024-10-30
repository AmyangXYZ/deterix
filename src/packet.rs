use crate::config::*;
use rand::Rng;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PacketType {
    Ack,
    TimeSync,
    JoinReq,
    JoinResp,
    Routing,
    Schedule,
    Statistics,
    Data,
}

impl From<u8> for PacketType {
    fn from(value: u8) -> Self {
        match value {
            0 => PacketType::Ack,
            1 => PacketType::TimeSync,
            2 => PacketType::JoinReq,
            3 => PacketType::JoinResp,
            4 => PacketType::Routing,
            5 => PacketType::Schedule,
            6 => PacketType::Statistics,
            7 => PacketType::Data,
            _ => panic!("Invalid PacketType value"),
        }
    }
}

pub enum PayloadView<'a> {
    Ack(AckView<'a>),
    TimeSync(TimeSyncView<'a>),
    JoinReq(JoinReqView<'a>),
    JoinResp(JoinRespView<'a>),
    Routing(RoutingView<'a>),
    Schedule(ScheduleView<'a>),
    Statistics(StatisticsView<'a>),
    Data(DataView<'a>),
}

pub struct PacketView<'a> {
    pub buffer: PacketBuffer<'a>,
}

impl<'a> PacketView<'a> {
    pub fn new(buffer: PacketBuffer<'a>) -> Self {
        Self { buffer }
    }
    pub fn magic(&self) -> u16 {
        u16::from_be_bytes(self.buffer.as_slice()[0..2].try_into().unwrap())
    }
    pub fn uid(&self) -> u64 {
        u64::from_be_bytes(self.buffer.as_slice()[2..10].try_into().unwrap())
    }
    pub fn ptype(&self) -> PacketType {
        PacketType::from(self.buffer.as_slice()[10])
    }
    pub fn src(&self) -> u32 {
        u32::from_be_bytes(self.buffer.as_slice()[11..15].try_into().unwrap())
    }
    pub fn dst(&self) -> u32 {
        u32::from_be_bytes(self.buffer.as_slice()[15..19].try_into().unwrap())
    }
    // pub fn priority(&self) -> u8 {
    //     self.buffer.as_slice()[19]
    // }
    // pub fn token(&self) -> u64 {
    //     u64::from_be_bytes(self.buffer.as_slice()[20..28].try_into().unwrap())
    // }
    // pub fn timestamp(&self) -> u64 {
    //     u64::from_be_bytes(self.buffer.as_slice()[28..36].try_into().unwrap())
    // }
    pub fn payload(&self) -> PayloadView {
        match self.ptype() {
            PacketType::Ack => PayloadView::Ack(AckView {
                payload: self.payload_slice(),
            }),
            PacketType::TimeSync => PayloadView::TimeSync(TimeSyncView {
                payload: self.payload_slice(),
            }),
            PacketType::JoinReq => PayloadView::JoinReq(JoinReqView {
                payload: self.payload_slice(),
            }),
            PacketType::JoinResp => PayloadView::JoinResp(JoinRespView {
                payload: self.payload_slice(),
            }),
            PacketType::Routing => PayloadView::Routing(RoutingView {
                payload: self.payload_slice(),
            }),
            PacketType::Schedule => PayloadView::Schedule(ScheduleView {
                payload: self.payload_slice(),
            }),
            PacketType::Statistics => PayloadView::Statistics(StatisticsView {
                payload: self.payload_slice(),
            }),
            PacketType::Data => PayloadView::Data(DataView {
                payload: self.payload_slice(),
            }),
        }
    }
    fn payload_slice(&self) -> &[u8] {
        &self.buffer.as_slice()[36..]
    }
}

pub struct PacketBuilder<'a> {
    buffer: PacketBuffer<'a>,
}

impl<'a> PacketBuilder<'a> {
    pub fn new(mut buffer: PacketBuffer<'a>) -> Self {
        buffer.as_mut_slice().fill(0);
        Self { buffer }
    }

    fn write_header(&mut self, ptype: PacketType, src: u32, dst: u32, priority: u8) {
        let header = self.buffer.as_mut_slice();
        header[0..2].copy_from_slice(&(MAGIC as u16).to_be_bytes());
        header[2..10].copy_from_slice(&(rand::thread_rng().gen_range(1..=u64::MAX)).to_be_bytes());
        header[10] = ptype as u8;
        header[11..15].copy_from_slice(&src.to_be_bytes());
        header[15..19].copy_from_slice(&dst.to_be_bytes());
        header[19] = priority;
        header[20..28].copy_from_slice(&TOKEN.to_be_bytes());
        header[28..36].copy_from_slice(
            &(SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64)
                .to_be_bytes(),
        );
    }

    fn write_ack_payload(&mut self, uid: u64) {
        self.buffer.as_mut_slice()[36..44].copy_from_slice(&uid.to_be_bytes());
        self.buffer.set_size(44);
    }

    fn write_data_payload(&mut self, data: &[u8]) {
        let payload = self.buffer.as_mut_slice();
        payload[36] = data.len() as u8;
        payload[37..37 + data.len()].copy_from_slice(data);
        self.buffer.set_size(37 + data.len());
    }

    pub fn new_ack(
        pool: &PacketBufferPool,
        src: u32,
        dst: u32,
        uid: u64,
    ) -> Option<PacketBuffer<'a>> {
        let buffer = pool.take()?;
        let mut packet_builder = Self::new(buffer);
        packet_builder.write_header(PacketType::Ack, src, dst, 0);
        packet_builder.write_ack_payload(uid);
        Some(packet_builder.buffer)
    }

    pub fn new_data(
        pool: &PacketBufferPool,
        src: u32,
        dst: u32,
        data: &[u8],
    ) -> Option<PacketBuffer<'a>> {
        let buffer = pool.take()?;
        let mut packet_builder = Self::new(buffer);
        packet_builder.write_header(PacketType::Data, src, dst, 0);
        packet_builder.write_data_payload(data);
        Some(packet_builder.buffer)
    }
}

pub struct AckView<'a> {
    payload: &'a [u8],
}

impl<'a> AckView<'a> {
    pub fn uid(&self) -> u64 {
        u64::from_be_bytes(self.payload[0..8].try_into().unwrap())
    }
}

pub struct TimeSyncView<'a> {
    payload: &'a [u8],
}

impl<'a> TimeSyncView<'a> {
    pub fn time(&self) -> u64 {
        u64::from_be_bytes(self.payload[0..8].try_into().unwrap())
    }
}

pub struct JoinReqView<'a> {
    payload: &'a [u8],
}

impl<'a> JoinReqView<'a> {
    pub fn id(&self) -> u32 {
        u32::from_be_bytes(self.payload[0..4].try_into().unwrap())
    }
}

pub struct JoinRespView<'a> {
    payload: &'a [u8],
}

impl<'a> JoinRespView<'a> {
    pub fn permitted(&self) -> u8 {
        self.payload[0]
    }
}

pub struct RoutingView<'a> {
    payload: &'a [u8],
}

impl<'a> RoutingView<'a> {
    pub fn entry(&self) -> u32 {
        u32::from_be_bytes(self.payload[0..4].try_into().unwrap())
    }
}

pub struct ScheduleView<'a> {
    payload: &'a [u8],
}

impl<'a> ScheduleView<'a> {
    pub fn entry(&self) -> u32 {
        u32::from_be_bytes(self.payload[0..4].try_into().unwrap())
    }
}

pub struct StatisticsView<'a> {
    payload: &'a [u8],
}

impl<'a> StatisticsView<'a> {
    pub fn entry(&self) -> u32 {
        u32::from_be_bytes(self.payload[0..4].try_into().unwrap())
    }
}

pub struct DataView<'a> {
    payload: &'a [u8],
}

impl<'a> DataView<'a> {
    pub fn name(&self) -> &str {
        std::str::from_utf8(&self.payload[0..128])
            .expect("Invalid UTF-8 sequence")
            .trim_end_matches('\0')
    }
    pub fn size(&self) -> usize {
        self.payload[128] as usize
    }
    pub fn data(&self) -> &[u8] {
        &self.payload[129..129 + self.size()]
    }
}

// Packet buffer pool implementation

pub struct Buffer {
    data: UnsafeCell<[u8; PACKET_SIZE]>,
    in_use: AtomicBool,
}

// Required to share Buffer between threads
unsafe impl Sync for Buffer {}

pub struct PacketBufferPool {
    buffers: Box<[Buffer]>,
}

pub struct PacketBuffer<'a> {
    buffer: &'a Buffer,
    size: usize,
}

impl PacketBufferPool {
    pub fn new() -> Self {
        let buffers = (0..PACKET_POOL_SIZE)
            .map(|_| Buffer {
                data: UnsafeCell::new([0; PACKET_SIZE]),
                in_use: AtomicBool::new(false),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self { buffers }
    }

    pub fn take(&self) -> Option<PacketBuffer<'static>> {
        for buf in self.buffers.iter() {
            if !buf.in_use.load(Ordering::Relaxed)
                && buf
                    .in_use
                    .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
            {
                return Some(PacketBuffer {
                    // SAFETY: The buffer lives as long as PacketBufferPool, which is 'static
                    buffer: unsafe { &*(buf as *const Buffer) },
                    size: PACKET_SIZE,
                });
            }
        }
        None
    }
}

impl<'a> PacketBuffer<'a> {
    pub fn as_slice(&self) -> &[u8] {
        unsafe { &*self.buffer.data.get() }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { &mut *self.buffer.data.get() }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.as_slice()[..self.size]
    }

    pub fn set_size(&mut self, size: usize) {
        self.size = size;
    }
}

impl<'a> Drop for PacketBuffer<'a> {
    fn drop(&mut self) {
        self.buffer.in_use.store(false, Ordering::Release);
    }
}
