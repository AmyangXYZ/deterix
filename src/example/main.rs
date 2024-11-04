use deterix::{Node, PayloadView};
use std::thread;
use std::time::Duration;
fn main() {
    let orchestrator = thread::spawn(move || {
        let mut orchestrator = Node::new_orchestrator("127.0.0.10:7777", false);
        orchestrator.serve();
    });

    let node1 = thread::spawn(move || {
        let mut node = Node::new(1, "127.0.0.11:7777", false);
        node.join("127.0.0.10:7777");
        println!("[Node 1] Joined");
        node.routing_table
            .insert(2, "127.0.0.12:7777".parse().unwrap());
        for i in 0..10 {
            node.send(2, format!("Hello world - {}", i).as_bytes());
            println!("[Node 1] Sending data: {}", i);
            // thread::sleep(Duration::from_secs(1));
        }
    });

    let node2 = thread::spawn(move || {
        let mut node = Node::new(2, "127.0.0.12:7777", false);
        node.join("127.0.0.10:7777");
        println!("[Node 2] Joined");

        for _ in 0..10 {
            if let Some((packet, _)) = node.recv(Duration::from_secs(5)) {
                if let PayloadView::Data(data) = packet.payload() {
                    println!("[Node 2] Received data: {:?}", data.data());
                }
            }
        }
    });

    orchestrator.join().expect("Orchestrator panicked");
    node1.join().expect("Node 1 panicked");
    node2.join().expect("Node 2 panicked");
}
