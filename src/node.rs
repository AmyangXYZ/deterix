use crate::config::*;
use crate::packet::*;
use crate::schedule::*;
// use core_affinity;
use std::collections::vec_deque::VecDeque;
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{sync_channel, Receiver};
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
    pub schedule: Arc<Mutex<Schedule>>,
    pub routing_table: HashMap<u32, SocketAddr>,
    is_orchestrator: bool,
    slot_duration: u64,
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

        if id == ORCHESTRATOR_ID {
            panic!("{} is reserved for orchestrator", ORCHESTRATOR_ID);
        }

        Self {
            id,
            is_orchestrator: false,
            joined: Arc::new(AtomicBool::new(false)),
            slot_duration: 0,
            reference_clock: Arc::new(AtomicU64::new(0)),
            routing_table: HashMap::new(),
            schedule: Arc::new(Mutex::new(Schedule::default())),
            pool: Arc::new(PacketBufferPool::new()),
            socket: UdpSocket::bind(address).unwrap(),
            tx_queue: Arc::new(Mutex::new(VecDeque::new())),
            rx_queue: Arc::new(Mutex::new(VecDeque::new())),
            verbose,
        }
    }

    pub fn new_orchestrator(
        address: &str,
        slot_duration: u64,
        schedule: Schedule,
        verbose: bool,
    ) -> Self {
        #[cfg(target_os = "linux")]
        Self::check_rt_scheduler(verbose);

        let mut orchestrator = Self {
            id: ORCHESTRATOR_ID,
            is_orchestrator: true,
            joined: Arc::new(AtomicBool::new(true)),
            reference_clock: Arc::new(AtomicU64::new(0)),
            slot_duration,
            routing_table: HashMap::new(),
            schedule: Arc::new(Mutex::new(schedule)),
            pool: Arc::new(PacketBufferPool::new()),
            socket: UdpSocket::bind(address).unwrap(),
            tx_queue: Arc::new(Mutex::new(VecDeque::new())),
            rx_queue: Arc::new(Mutex::new(VecDeque::new())),
            verbose,
        };

        orchestrator.run(slot_duration);
        orchestrator
    }

    // core logic, send/receive packets following a TDMA schedule
    fn run(&mut self, slot_duration: u64) {
        if self.is_orchestrator {
            println!(
                "*** New DETERIX Orchestrator running on {} ***",
                self.socket.local_addr().unwrap()
            );
        } else {
            println!(
                "*** New DETERIX node [{}] running on {} ***",
                self.id,
                self.socket.local_addr().unwrap()
            );
        }

        let socket = self.socket.try_clone().expect("Failed to clone socket");
        let pool = Arc::clone(&self.pool);

        let schedule = Arc::clone(&self.schedule);
        let tx_queue = Arc::clone(&self.tx_queue);
        let rx_queue = Arc::clone(&self.rx_queue);
        let id = self.id;
        let verbose = self.verbose;
        let slot_ticker = self.create_slot_ticker(slot_duration);

        thread::spawn(move || {
            #[cfg(target_os = "linux")]
            if RT_SCHEDULER_AVAILABLE.load(Ordering::Relaxed) {
                unsafe {
                    let mut sched_param: libc::sched_param = std::mem::zeroed();
                    sched_param.sched_priority = THREAD_PRIORITY_PACKET_QUEUE;

                    if libc::pthread_setschedparam(
                        libc::pthread_self(),
                        libc::SCHED_FIFO,
                        &sched_param,
                    ) != 0
                    {
                        eprintln!("Failed to set SCHED_RR. Error: {}", Error::last_os_error());
                        return;
                    }
                }
            }

            let clear_window = slot_duration * 1 / 10;
            let tx_window = slot_duration * 5 / 10;
            let ack_window = slot_duration * 3 / 10;
            let guard_band = slot_duration * 1 / 10;
            let slotframe_size = schedule.lock().unwrap().slots.len();
            while let Ok((slot_number, slot_start_time)) = slot_ticker.recv() {
                let slot =
                    schedule.lock().unwrap().slots[slot_number as usize % slotframe_size as usize];

                // println!("[Node {}] *Slot {}* {:?}", id, slot_number, slot_start_time);

                let slot_start_time = UNIX_EPOCH + Duration::from_micros(slot_start_time);

                let clear_end_time = slot_start_time + Duration::from_micros(clear_window);
                let tx_end_time = clear_end_time + Duration::from_micros(tx_window);
                let ack_end_time = tx_end_time + Duration::from_micros(ack_window);
                let slot_end_time = ack_end_time + Duration::from_micros(guard_band);
                // println!(
                //     "[Node {}] *Slot End times* {:?} {:?} {:?} {:?}",
                //     id,
                //     clear_end_time
                //         .duration_since(UNIX_EPOCH)
                //         .unwrap()
                //         .as_micros(),
                //     tx_end_time.duration_since(UNIX_EPOCH).unwrap().as_micros(),
                //     ack_end_time.duration_since(UNIX_EPOCH).unwrap().as_micros(),
                //     slot_end_time
                //         .duration_since(UNIX_EPOCH)
                //         .unwrap()
                //         .as_micros()
                // );

                // if verbose {
                // }

                // CLEAR WINDOW: clear packet in socket buffer from previous slots
                {
                    let _ =
                        socket.set_read_timeout(Some(Duration::from_micros(clear_window * 4 / 5)));
                    if let Some(mut temp_packet_buffer) = pool.take() {
                        let _ = socket.recv_from(&mut temp_packet_buffer.as_mut_slice());
                    }
                    Self::sleep_until(clear_end_time);
                }

                // TX/RX and ACK WINDOW: transmit/receive a packet and wait/send ack
                {
                    if slot.sender == id {
                        let Some((packet_buffer, addr)) = tx_queue.lock().unwrap().pop_front()
                        else {
                            continue;
                        };
                        let packet_view = PacketView::new(packet_buffer);
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

                        let _ = socket.send_to(packet_view.buffer.as_bytes(), addr);

                        let ptype = packet_view.ptype();
                        let uid = packet_view.uid();
                        let dst = packet_view.dst();
                        if verbose {
                            println!(
                                "[Node {}] *Slot {}* Sent {:?} to {}",
                                id, slot_number, ptype, dst
                            );
                        }

                        // wait for ack
                        let _ = socket.set_read_timeout(Some(Duration::from_micros(ack_window)));
                        Self::sleep_until(tx_end_time);

                        if let Some(mut ack_buffer) = pool.take() {
                            if let Ok((_, _)) = socket.recv_from(&mut ack_buffer.as_mut_slice()) {
                                let ack_view = PacketView::new(ack_buffer);

                                if ack_view.magic() == MAGIC
                                    && ack_view.ptype() == PacketType::Ack
                                    && ack_view.dst() == id
                                    && ack_view.src() == packet_view.dst()
                                {
                                    if let PayloadView::Ack(ack) = ack_view.payload() {
                                        if ack.uid() == uid {
                                            if verbose {
                                                println!(
                                                    "[Node {}] *Slot {}* Ack received",
                                                    id, slot_number
                                                );
                                            }
                                        }
                                    }
                                }
                            } else {
                                if verbose {
                                    println!("[Node {}] Ack timeout", id);
                                    println!("[Node {}] Resent {:?} to {}", id, ptype, dst);
                                }
                                // resend the packet immediately at the next slot
                                tx_queue
                                    .lock()
                                    .unwrap()
                                    .push_front((packet_view.buffer, addr));
                            }

                            Self::sleep_until(ack_end_time);
                        }
                    } else if slot.receiver == id {
                        let _ = socket.set_read_timeout(Some(Duration::from_micros(tx_window)));

                        if let Some(mut packet_buffer) = pool.take() {
                            if let Ok((_, src)) =
                                socket.recv_from(&mut packet_buffer.as_mut_slice())
                            {
                                let packet_view = PacketView::new(packet_buffer);
                                let uid = packet_view.uid();

                                if packet_view.magic() == MAGIC
                                    && packet_view.dst() == id
                                    && (packet_view.src() == slot.sender || slot.sender == ANY_NODE)
                                    && packet_view.ptype() != PacketType::Ack
                                {
                                    if verbose {
                                        println!(
                                            "[Node {}] *Slot {}* Received {:?}",
                                            id,
                                            slot_number,
                                            packet_view.ptype()
                                        );
                                        println!(
                                            "[Node {}] *Slot {}* Responded ack",
                                            id, slot_number
                                        );
                                    }

                                    rx_queue
                                        .lock()
                                        .unwrap()
                                        .push_back((packet_view.buffer, src));

                                    if let Some(ack) = PacketBuilder::new_ack(
                                        &pool,
                                        slot.receiver,
                                        slot.sender,
                                        uid,
                                    ) {
                                        Self::sleep_until(tx_end_time);
                                        let _ = socket.send_to(ack.as_bytes(), src);
                                        Self::sleep_until(ack_end_time);
                                    }
                                }
                            }
                        }
                    }
                }

                // GUARD BAND: wait for guard band
                {
                    Self::sleep_until(slot_end_time);
                }
            }
        });

        // core_affinity::set_for_current(core_affinity::CoreId { id: id as usize });
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

    fn create_slot_ticker(&self, slot_duration: u64) -> Receiver<(u64, u64)> {
        let (slot_ticker_sender, slot_ticker_receiver) = sync_channel::<(u64, u64)>(0);
        let reference_clock = Arc::clone(&self.reference_clock);
        let is_orchestrator = self.is_orchestrator;
        // let id = self.id;
        thread::spawn(move || {
            // core_affinity::set_for_current(core_affinity::CoreId { id: id as usize });

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

            if is_orchestrator {
                reference_clock.store(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as u64,
                    Ordering::Relaxed,
                );
                println!(
                    "[Orchestrator] Sync reference clock: {}",
                    reference_clock.load(Ordering::Relaxed) as f64 / 1e6
                );
            }

            loop {
                let reference_time = reference_clock.load(Ordering::Relaxed);

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_micros() as u64;
                if (now - reference_time) % slot_duration == 0 {
                    slot_ticker_sender
                        .send(((now - reference_time) / slot_duration, now))
                        .expect("Failed to send tick");
                    // thread::sleep(Duration::from_micros(SLOT_DURATION * 4 / 5));
                }
                #[cfg(target_os = "linux")]
                if RT_SCHEDULER_AVAILABLE.load(Ordering::Relaxed) {
                    unsafe {
                        libc::sched_yield();
                    }
                } else {
                    thread::yield_now();
                }

                #[cfg(not(target_os = "linux"))]
                thread::yield_now();
            }
        });

        slot_ticker_receiver
    }

    fn sleep_until(deadline: SystemTime) {
        while SystemTime::now() < deadline {
            #[cfg(target_os = "linux")]
            if RT_SCHEDULER_AVAILABLE.load(Ordering::Relaxed) {
                unsafe {
                    libc::sched_yield();
                }
            } else {
                thread::yield_now();
            }

            #[cfg(not(target_os = "linux"))]
            thread::yield_now();
        }
    }
    fn enqueue_tx_packet(&mut self, packet: PacketBuffer<'static>, addr: SocketAddr) {
        self.tx_queue.lock().unwrap().push_back((packet, addr));
    }

    fn dequeue_rx_packet(&mut self) -> Option<(PacketBuffer<'static>, SocketAddr)> {
        self.rx_queue.lock().unwrap().pop_front()
    }

    /// Receive a single packet
    pub fn recv(&mut self, timeout: Duration) -> Option<(PacketView<'static>, SocketAddr)> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some((buffer, addr)) = self.dequeue_rx_packet() {
                return Some((PacketView::new(buffer), addr));
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

    // use non-TDMA protocol to join the network
    pub fn join(&mut self, orchestrator_address: &str) {
        if self.is_orchestrator {
            panic!("Orchestrator does not need to join the network");
        }
        self.routing_table.insert(
            ORCHESTRATOR_ID,
            orchestrator_address.parse().expect("Invalid address"),
        );

        if self.verbose {
            println!("[Node {}] Sending join request", self.id);
        }
        loop {
            let join_req =
                PacketBuilder::new_join_req(&self.pool, self.id, ORCHESTRATOR_ID, self.id).unwrap();

            self.socket
                .send_to(
                    join_req.as_bytes(),
                    &self
                        .routing_table
                        .get(&ORCHESTRATOR_ID)
                        .expect("Destination not in routing table")
                        .clone(),
                )
                .expect("Failed to send join request");

            let _ = self
                .socket
                .set_read_timeout(Some(Duration::from_micros(1000)));
            if let Some(mut packet_buffer) = self.pool.take() {
                if let Ok((_, _src)) = self.socket.recv_from(&mut packet_buffer.as_mut_slice()) {
                    let packet_view = PacketView::new(packet_buffer);
                    if packet_view.magic() == MAGIC && packet_view.ptype() == PacketType::Ack {
                        break;
                    }
                }
            }
        }

        loop {
            let _ = self.socket.set_read_timeout(Some(Duration::from_secs(5)));
            if let Some(mut packet_buffer) = self.pool.take() {
                if let Ok((_, _src)) = self.socket.recv_from(&mut packet_buffer.as_mut_slice()) {
                    let packet_view = PacketView::new(packet_buffer);
                    let uid = packet_view.uid();
                    if packet_view.magic() == MAGIC {
                        if let PayloadView::JoinResp(join_resp) = packet_view.payload() {
                            if join_resp.permitted() == 1 {
                                self.joined.store(true, Ordering::Relaxed);
                                self.slot_duration = join_resp.slot_duration();
                                self.reference_clock
                                    .store(join_resp.reference_clock(), Ordering::Relaxed);
                                *self.schedule.lock().unwrap() = join_resp.schedule();

                                if self.verbose {
                                    println!(
                                        "[Node {}] Join response received, sync clock and schedule",
                                        self.id
                                    );
                                }

                                if let Some(ack) = PacketBuilder::new_ack(
                                    &self.pool,
                                    self.id,
                                    ORCHESTRATOR_ID,
                                    uid,
                                ) {
                                    let _ = self.socket.send_to(
                                        ack.as_bytes(),
                                        &self
                                            .routing_table
                                            .get(&ORCHESTRATOR_ID)
                                            .expect("Destination not in routing table")
                                            .clone(),
                                    );
                                }
                                self.run(self.slot_duration);
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Orchestrator only, serve join requests and synchronize schedules
    pub fn serve(&mut self) {
        if !self.is_orchestrator {
            panic!("Only orchestrator can serve join requests");
        }

        loop {
            if let Some((packet, addr)) = self.recv(Duration::from_secs(5)) {
                if packet.ptype() == PacketType::JoinReq {
                    // {
                    //     let mut schedule = self.schedule.lock().unwrap();
                    //     schedule.slots[2] =
                    //         Slot::new_dedicate(self.id, packet.src(), ORCHESTRATOR_ID);
                    // }
                    let join_resp = PacketBuilder::new_join_resp(
                        &self.pool,
                        self.id,
                        packet.src(),
                        1,
                        self.slot_duration,
                        self.reference_clock.load(Ordering::Relaxed),
                        &self.schedule.lock().unwrap(),
                    )
                    .unwrap();

                    self.routing_table.insert(packet.src(), addr);

                    self.enqueue_tx_packet(join_resp, addr);
                }
            }
        }
    }
}
