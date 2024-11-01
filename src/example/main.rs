use deterix::{Node, PayloadView};
use std::thread;
use std::time::Duration;
fn main() {
    let orchestrator = thread::spawn(move || {
        let mut orchestrator = Node::new_orchestrator("127.0.0.1:7778", false);
        orchestrator.serve();
    });

    let node1 = thread::spawn(move || {
        let mut node = Node::new(1, "127.0.0.1:7777", false);
        node.join("127.0.0.1:7778");
        println!("[Node 1] Joined");
        if let Some((packet, _)) = node.recv(Duration::from_secs(5)) {
            if let PayloadView::Data(data) = packet.payload() {
                println!("[Node 1] Received data: {:?}", data.data());
            }
        } else {
            println!("[Node 1] No data received");
        }
    });

    let node2 = thread::spawn(move || {
        let mut node = Node::new(2, "127.0.0.2:7777", false);
        node.join("127.0.0.1:7778");
        println!("[Node 2] Joined");
        node.routing_table
            .insert(1, "127.0.0.1:7777".parse().unwrap());
        node.send(1, "Hello".as_bytes());
        println!("[Node 2] Sending data");
    });

    orchestrator.join().expect("Orchestrator panicked");
    node1.join().expect("Node 1 panicked");
    node2.join().expect("Node 2 panicked");
}
