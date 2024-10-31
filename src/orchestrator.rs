use crate::config::*;
use std::net::{SocketAddr, UdpSocket};

pub struct Orchestrator {
    socket: UdpSocket,
}

impl Orchestrator {
    pub fn new(listen_addr: SocketAddr) -> Self {
        let socket = UdpSocket::bind(listen_addr).unwrap();
        let mut orchestrator = Self { socket };
        orchestrator.run();
        orchestrator
    }

    fn run(&mut self) {
        loop {
            let mut buffer = [0; PACKET_SIZE];
            let (amt, _) = self.socket.recv_from(&mut buffer).unwrap();
        }
    }
}
