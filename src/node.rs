use crate::config::*;
use crate::packet::*;
use crate::schedule::*;
use core_affinity;
use std::collections::vec_deque::VecDeque;
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use {
    libc,
    std::fs,
    std::io::Error,
    std::sync::atomic::{AtomicBool, Ordering},
};

#[cfg(target_os = "linux")]
static RT_SCHEDULER_AVAILABLE: AtomicBool = AtomicBool::new(false);

pub struct Node {
    pub id: u32,
    pub pool: Arc<PacketBufferPool>,
    pub schedule: Arc<Schedule>,
    socket: UdpSocket,
    tx_queue: Arc<Mutex<VecDeque<(PacketBuffer<'static>, SocketAddr)>>>,
    rx_queue: Arc<Mutex<VecDeque<(PacketBuffer<'static>, SocketAddr)>>>,
    verbose: bool,
}

impl Node {
    pub fn new(id: u32, address: &str, verbose: bool) -> Self {
        #[cfg(target_os = "linux")]
        Self::check_rt_scheduler(verbose);

        let mut node = Self {
            id,
            pool: Arc::new(PacketBufferPool::new()),
            schedule: Arc::new(Schedule::new()),
            socket: UdpSocket::bind(address).unwrap(),
            tx_queue: Arc::new(Mutex::new(VecDeque::new())),
            rx_queue: Arc::new(Mutex::new(VecDeque::new())),
            verbose,
        };

        node.run();
        node
    }

    // core logic, send/receive packets following a TDMA schedule
    fn run(&mut self) {
        println!(
            "*** New RT-NDS node [{}] running on {} ***",
            self.id,
            self.socket.local_addr().unwrap()
        );

        let socket = self.socket.try_clone().expect("Failed to clone socket");

        let _ = socket
            .set_read_timeout(Some(std::time::Duration::from_micros(ACK_TIMEOUT)))
            .expect("Failed to set read timeout");

        let pool = Arc::clone(&self.pool);
        let schedule = Arc::clone(&self.schedule);
        let tx_queue = Arc::clone(&self.tx_queue);
        let rx_queue = Arc::clone(&self.rx_queue);
        let id = self.id;
        let verbose = self.verbose;

        thread::spawn(move || {
            #[cfg(target_os = "linux")]
            if RT_SCHEDULER_AVAILABLE.load(Ordering::Relaxed) {
                unsafe {
                    let mut sched_param: libc::sched_param = std::mem::zeroed();
                    sched_param.sched_priority = THREAD_PRIORITY_PACKET_QUEUE;

                    if libc::pthread_setschedparam(
                        libc::pthread_self(),
                        libc::SCHED_RR,
                        &sched_param,
                    ) != 0
                    {
                        eprintln!("Failed to set SCHED_RR. Error: {}", Error::last_os_error());
                        return;
                    }
                }
            }

            let slot_ticker = Self::create_slot_ticker(id);

            while let Ok(slot_number) = slot_ticker.recv() {
                let slot = schedule.slots[slot_number as usize % SLOTFRAME_SIZE as usize];
                if verbose {
                    println!("[Node {}] Slot-{:?} {:?}", id, slot_number, slot);
                }
                if slot.sender == id {
                    let Some((packet_buffer, addr)) = tx_queue.lock().unwrap().pop_front() else {
                        continue;
                    };
                    let _ = socket.send_to(packet_buffer.as_bytes(), addr);
                    let packet_view = PacketView::new(packet_buffer);
                    let ptype = packet_view.ptype();
                    let uid = packet_view.uid();
                    if verbose {
                        println!("[Node {}] Sent {:?} to {}", id, ptype, slot.receiver);
                    }
                    // wait for ack
                    if let Some(mut ack_buffer) = pool.take() {
                        if let Ok((_, _)) = socket.recv_from(&mut ack_buffer.as_mut_slice()) {
                            let packet_view = PacketView::new(ack_buffer);

                            if packet_view.magic() == MAGIC
                                && packet_view.ptype() == PacketType::Ack
                                && packet_view.dst() == id
                                && packet_view.src() == slot.receiver
                            {
                                if let PayloadView::Ack(ack) = packet_view.payload() {
                                    if ack.uid() == uid {
                                        if verbose {
                                            println!("[Node {}] Ack received", id);
                                        }
                                    }
                                }
                            }
                        } else {
                            if verbose {
                                println!("[Node {}] Ack timeout", id);
                                println!("[Node {}] Resent {:?} to {}", id, ptype, slot.receiver);
                            }
                            // resend the packet immediately at the next slot
                            tx_queue
                                .lock()
                                .unwrap()
                                .push_front((packet_view.buffer, addr));
                        }
                    }
                } else if slot.receiver == id {
                    if let Some(mut packet_buffer) = pool.take() {
                        if let Ok((_, src)) = socket.recv_from(&mut packet_buffer.as_mut_slice()) {
                            let packet_view = PacketView::new(packet_buffer);
                            let uid = packet_view.uid();

                            if packet_view.magic() == MAGIC && packet_view.dst() == id {
                                if verbose {
                                    println!("[Node {}] Received {:?}", id, packet_view.ptype());
                                }

                                if let Some(ack) =
                                    PacketBuilder::new_ack(&pool, slot.receiver, slot.sender, uid)
                                {
                                    let _ = socket.send_to(ack.as_bytes(), src);
                                    if verbose {
                                        println!("[Node {}] Responded ack", id);
                                    }
                                }

                                rx_queue
                                    .lock()
                                    .unwrap()
                                    .push_back((packet_view.buffer, src));
                            }
                        }
                    }
                }
            }
        });

        if let Some(core_id) = core_affinity::get_core_ids()
            .and_then(|ids| ids.get(CORE_AFFINITY_PACKET_QUEUE).cloned())
        {
            core_affinity::set_for_current(core_id);
        } else {
            eprintln!("[Server] Failed to set core affinity for packet queue");
        }
    }

    #[cfg(target_os = "linux")]
    fn check_rt_scheduler(verbose: bool) {
        if let Ok(contents) = fs::read_to_string("/sys/kernel/realtime") {
            if contents.trim() == "1" {
                RT_SCHEDULER_AVAILABLE.store(true, Ordering::Relaxed);
                if verbose {
                    println!("[Server] PREEMPT-RT patch available, set thread priorities");
                }
            }
        } else {
            if verbose {
                println!("[Server] PREEMPT-RT patch not available, no priority boosting");
            }
        }
    }

    fn create_slot_ticker(id: u32) -> Receiver<u64> {
        let (slot_ticker_sender, slot_ticker_receiver) = channel::<u64>();

        thread::spawn(move || {
            // Set thread affinity to a core
            if let Some(core_id) = core_affinity::get_core_ids()
                .and_then(|ids| ids.get(id as usize + CORE_AFFINITY_SLOT_TICKER).cloned())
            {
                core_affinity::set_for_current(core_id);
            } else {
                eprintln!("[Node {}] Failed to set core affinity for slot ticker", id);
            }

            #[cfg(target_os = "linux")]
            if RT_SCHEDULER_AVAILABLE.load(Ordering::Relaxed) {
                unsafe {
                    let mut sched_param: libc::sched_param = std::mem::zeroed();
                    sched_param.sched_priority = THREAD_PRIORITY_SLOT_TICKER;

                    if libc::pthread_setschedparam(
                        libc::pthread_self(),
                        libc::SCHED_RR,
                        &sched_param,
                    ) != 0
                    {
                        eprintln!("Failed to set SCHED_RR. Error: {}", Error::last_os_error());
                        return;
                    }
                }
            }
            let mut absolute_slot_number = 0;
            let mut last_tick = Instant::now();
            loop {
                let now = Instant::now();
                if now >= last_tick + Duration::from_micros(SLOT_DURATION) {
                    slot_ticker_sender
                        .send(absolute_slot_number)
                        .expect("Failed to send tick");
                    last_tick = now;
                    absolute_slot_number += 1;
                }
            }
        });

        slot_ticker_receiver
    }

    fn enqueue_tx_packet(&mut self, packet: PacketBuffer<'static>, addr: SocketAddr) {
        self.tx_queue.lock().unwrap().push_back((packet, addr));
    }

    fn dequeue_rx_packet(&mut self) -> Option<(PacketBuffer<'static>, SocketAddr)> {
        self.rx_queue.lock().unwrap().pop_front()
    }

    /// Receive a single packet and call the callback with the packet
    pub fn recv_once(&mut self, callback: PacketCallback) {
        loop {
            if let Some((buffer, _addr)) = self.dequeue_rx_packet() {
                let packet = PacketView::new(buffer);
                callback(packet);
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Receive packets and call the callback with each packet
    pub fn recv(&mut self, callback: PacketCallback) {
        loop {
            self.recv_once(callback);
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Send a packet without waiting for reply
    pub fn send(&mut self, packet: PacketBuffer<'static>, addr: SocketAddr) {
        self.enqueue_tx_packet(packet, addr);
    }

    /// Send a packet and wait for reply
    pub fn send_await(
        &mut self,
        packet: PacketBuffer<'static>,
        addr: SocketAddr,
        callback: PacketCallback,
    ) {
        self.enqueue_tx_packet(packet, addr);
        loop {
            if let Some((packet, _addr)) = self.dequeue_rx_packet() {
                let packet_view = PacketView::new(packet);
                if packet_view.magic() == MAGIC && packet_view.dst() == self.id {
                    callback(packet_view);
                    break;
                }
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}
