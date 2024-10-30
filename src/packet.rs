use crate::config::*;
use rand::Rng;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub type PacketCallback = fn(PacketView);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PacketType {
    Ack,
    Status,
    Data,
}

impl From<u8> for PacketType {
    fn from(value: u8) -> Self {
        match value {
            0 => PacketType::Ack,
            1 => PacketType::Status,
            2 => PacketType::Data,
            _ => panic!("Invalid PacketType value"),
        }
    }
}

pub enum PayloadView<'a> {
    Ack(AckView<'a>),
    Status(StatusView<'a>),
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
            PacketType::Status => PayloadView::Status(StatusView {
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

    fn write_status_payload(&mut self, code: StatusCode, message: &str) {
        self.buffer.as_mut_slice()[36] = code as u8;
        self.buffer.as_mut_slice()[37..37 + message.len()].copy_from_slice(message.as_bytes());
        self.buffer.set_size(37 + message.len());
    }

    fn write_data_payload(&mut self, name: &str, data: &[u8]) {
        let payload = self.buffer.as_mut_slice();
        payload[36..36 + name.len()].copy_from_slice(name.as_bytes());
        payload[164] = data.len() as u8;
        payload[165..165 + data.len()].copy_from_slice(data);
        self.buffer.set_size(165 + data.len());
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

    pub fn new_status(
        pool: &PacketBufferPool,
        src: u32,
        dst: u32,
        code: StatusCode,
        message: &str,
    ) -> Option<PacketBuffer<'a>> {
        let buffer = pool.take()?;
        let mut packet_builder = Self::new(buffer);
        packet_builder.write_header(PacketType::Status, src, dst, 0);
        packet_builder.write_status_payload(code, message);
        Some(packet_builder.buffer)
    }

    pub fn new_data(
        pool: &PacketBufferPool,
        src: u32,
        dst: u32,
        name: &str,
        data: &[u8],
    ) -> Option<PacketBuffer<'a>> {
        let buffer = pool.take()?;
        let mut packet_builder = Self::new(buffer);
        packet_builder.write_header(PacketType::Data, src, dst, 0);
        packet_builder.write_data_payload(name, data);
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusCode {
    Success,
    ChecksumMismatch,
}

impl From<u8> for StatusCode {
    fn from(value: u8) -> Self {
        match value {
            0 => StatusCode::Success,
            1 => StatusCode::ChecksumMismatch,
            _ => panic!("Invalid StatusCode value"),
        }
    }
}

pub struct StatusView<'a> {
    payload: &'a [u8],
}

impl<'a> StatusView<'a> {
    pub fn code(&self) -> StatusCode {
        StatusCode::from(self.payload[0])
    }

    pub fn message(&self) -> &str {
        std::str::from_utf8(&self.payload[1..129])
            .unwrap()
            .trim_end_matches('\0')
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
