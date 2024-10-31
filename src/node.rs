use crate::config::*;
use crate::packet::*;
use crate::schedule::*;
use core_affinity;
use std::collections::vec_deque::VecDeque;
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
    pub schedule: Arc<Schedule>,
    pub routing_table: HashMap<u32, SocketAddr>,
    is_orchestrator: bool,
    joined: Arc<AtomicBool>,
    reference_clock: Arc<AtomicU64>,
    pool: Arc<PacketBufferPool>,
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
            is_orchestrator: id == ORCHESTRATOR_ID,
            joined: Arc::new(AtomicBool::new(id == ORCHESTRATOR_ID)),
            reference_clock: Arc::new(AtomicU64::new(0)),
            routing_table: HashMap::new(),
            schedule: Arc::new(Schedule::new()),
            pool: Arc::new(PacketBufferPool::new()),
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
            "*** New DETERIX node [node_{}] running on {} ***",
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
        let slot_ticker = self.create_slot_ticker();
        let joined = Arc::clone(&self.joined);

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

            while let Ok(slot_number) = slot_ticker.recv() {
                let slot = schedule.slots[slot_number as usize % SLOTFRAME_SIZE as usize];

                if verbose {
                    println!("[Node {}] *Slot {}*", id, slot_number);
                }

                if slot.sender == id || (!joined.load(Ordering::Relaxed) && slot.sender == ANY_NODE)
                {
                    let Some((packet_buffer, addr)) = tx_queue.lock().unwrap().pop_front() else {
                        continue;
                    };
                    let mut packet_view = PacketView::new(packet_buffer);
                    if packet_view.src() == id
                        && (slot.receiver == packet_view.dst() || (slot.receiver == ANY_NODE))
                    {
                    } else {
                        tx_queue
                            .lock()
                            .unwrap()
                            .push_front((packet_view.buffer, addr));
                        continue;
                    }

                    packet_view.buffer.set_slot_number(slot_number);
                    let _ = socket.send_to(packet_view.buffer.as_bytes(), addr);

                    let ptype = packet_view.ptype();
                    let uid = packet_view.uid();
                    if verbose {
                        println!(
                            "[Node {}] *Slot {}* Sent {:?} to {}",
                            id,
                            packet_view.slot_number(),
                            ptype,
                            slot.receiver
                        );
                    }

                    // wait for ack
                    if let Some(mut ack_buffer) = pool.take() {
                        if let Ok((_, _)) = socket.recv_from(&mut ack_buffer.as_mut_slice()) {
                            let ack_view = PacketView::new(ack_buffer);

                            if ack_view.magic() == MAGIC
                                && ack_view.ptype() == PacketType::Ack
                                && ack_view.dst() == id
                                && ack_view.src() == packet_view.dst()
                                && ack_view.slot_number() == slot_number
                            {
                                if let PayloadView::Ack(ack) = ack_view.payload() {
                                    if ack.uid() == uid {
                                        if verbose {
                                            println!(
                                                "[Node {}] *Slot {}* Ack received",
                                                id,
                                                ack_view.slot_number()
                                            );
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
                } else if slot.receiver == id
                    || (!joined.load(Ordering::Relaxed) && slot.receiver == ANY_NODE)
                {
                    if let Some(mut packet_buffer) = pool.take() {
                        if let Ok((_, src)) = socket.recv_from(&mut packet_buffer.as_mut_slice()) {
                            let packet_view = PacketView::new(packet_buffer);
                            let uid = packet_view.uid();

                            if packet_view.magic() == MAGIC
                                && packet_view.dst() == id
                                && (packet_view.src() == slot.sender || slot.sender == ANY_NODE)
                                && packet_view.ptype() != PacketType::Ack
                                && packet_view.slot_number() == slot_number
                            {
                                if verbose {
                                    println!(
                                        "[Node {}] *Slot {}* Received {:?}",
                                        id,
                                        slot_number,
                                        packet_view.ptype()
                                    );
                                }

                                if let Some(mut ack) =
                                    PacketBuilder::new_ack(&pool, slot.receiver, slot.sender, uid)
                                {
                                    ack.set_slot_number(slot_number);
                                    let _ = socket.send_to(ack.as_bytes(), src);
                                    if verbose {
                                        println!(
                                            "[Node {}] *Slot {}* Responded ack",
                                            id, slot_number
                                        );
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
            eprintln!("! Failed to set core affinity for packet queue");
        }
    }

    #[cfg(target_os = "linux")]
    fn check_rt_scheduler(verbose: bool) {
        if let Ok(contents) = fs::read_to_string("/sys/kernel/realtime") {
            if contents.trim() == "1" {
                RT_SCHEDULER_AVAILABLE.store(true, Ordering::Relaxed);
                if verbose {
                    println!("! PREEMPT-RT patch available, set thread priorities");
                }
            }
        } else {
            if verbose {
                println!("! PREEMPT-RT patch not available, no priority boosting");
            }
        }
    }

    fn create_slot_ticker(&self) -> Receiver<u64> {
        let (slot_ticker_sender, slot_ticker_receiver) = channel::<u64>();
        let reference_clock = Arc::clone(&self.reference_clock);
        let is_orchestrator = self.is_orchestrator;
        let id = self.id;

        thread::spawn(move || {
            // Set thread affinity to a core
            if let Some(core_id) = core_affinity::get_core_ids()
                .and_then(|ids| ids.get(id as usize + CORE_AFFINITY_SLOT_TICKER).cloned())
            {
                core_affinity::set_for_current(core_id);
            } else {
                eprintln!("! Failed to set core affinity for slot ticker");
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
                        eprintln!(
                            "! Failed to set SCHED_RR. Error: {}",
                            Error::last_os_error()
                        );
                        return;
                    }
                }
            }

            let mut absolute_slot_number = 0;

            if is_orchestrator {
                reference_clock.store(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as u64,
                    Ordering::Relaxed,
                );
            }

            loop {
                let reference_time = reference_clock.load(Ordering::Relaxed);

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_micros() as u64;
                if (now - reference_time) % SLOT_DURATION == 0 {
                    slot_ticker_sender
                        .send(absolute_slot_number % SLOTFRAME_SIZE as u64)
                        .expect("Failed to send tick");
                    absolute_slot_number += 1;

                    if is_orchestrator
                        && absolute_slot_number % REFERENCE_CLOCK_RESET_INTERVAL as u64 == 0
                    {
                        reference_clock.store(now, Ordering::Relaxed);
                    }
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

    /// Receive a single packet
    pub fn recv(&mut self, timeout: Duration) -> Option<PacketView<'static>> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some((buffer, _addr)) = self.dequeue_rx_packet() {
                return Some(PacketView::new(buffer));
            }
            thread::sleep(Duration::from_micros(10));
        }
        None
    }

    /// Send (enqueue) a data packet
    pub fn send(&mut self, dst: u32, data: &[u8]) {
        let packet = PacketBuilder::new_data(&self.pool, self.id, dst, data).unwrap();
        self.enqueue_tx_packet(
            packet,
            self.routing_table
                .get(&dst)
                .expect("Destination not in routing table")
                .clone(),
        );
    }

    pub fn join(&mut self) {
        let join_req =
            PacketBuilder::new_join_req(&self.pool, self.id, ORCHESTRATOR_ID, self.id).unwrap();

        self.enqueue_tx_packet(
            join_req,
            self.routing_table
                .get(&ORCHESTRATOR_ID)
                .expect("Destination not in routing table")
                .clone(),
        );
        println!("Sent join request to {}", ORCHESTRATOR_ID);

        let packet = self
            .recv(Duration::from_secs(10))
            .expect("No join response received");

        if let PayloadView::JoinResp(join_resp) = packet.payload() {
            if join_resp.permitted() == 1 {
                self.joined.store(true, Ordering::Relaxed);
                self.reference_clock
                    .store(join_resp.reference_clock(), Ordering::Relaxed);
                println!(
                    "Join response received from {}, reference clock set to {}",
                    packet.src(),
                    join_resp.reference_clock()
                );
            }
        }
    }

    pub fn respond_join_req(&mut self, permitted: u8) {
        let packet = self
            .recv(Duration::from_secs(10))
            .expect("No join response received");

        if let PayloadView::JoinReq(join_req) = packet.payload() {
            let join_resp = PacketBuilder::new_join_resp(
                &self.pool,
                self.id,
                packet.src(),
                permitted,
                self.reference_clock.load(Ordering::Relaxed),
            )
            .unwrap();
            self.enqueue_tx_packet(
                join_resp,
                self.routing_table
                    .get(&join_req.id())
                    .expect("Destination not in routing table")
                    .clone(),
            );
        }
    }
}
