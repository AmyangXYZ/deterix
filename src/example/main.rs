use deterix::{Node, PayloadView};
use std::thread;
use std::time::Duration;

fn main() {
    let orchestrator = thread::spawn(move || {
        let mut orchestrator = Node::new(0, "localhost:7778", true);
        orchestrator
            .routing_table
            .insert(1, "127.0.0.1:7777".parse().unwrap());
        orchestrator.respond_join_req(1);
        println!("Waiting for data");
        let packet = orchestrator.recv(Duration::from_secs(10)).unwrap();

        if let PayloadView::Data(data) = packet.payload() {
            println!("Data received: {:?}", data.data());
        }
    });

    let node1 = thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        let mut node = Node::new(1, "localhost:7777", true);
        node.routing_table
            .insert(0, "127.0.0.1:7778".parse().unwrap());

        node.join();
        println!("Node 1 joined");
        node.send(0, "Hello".as_bytes());
        println!("Sending data");
    });

    orchestrator.join().expect("Orchestrator panicked");
    node1.join().expect("Node 1 panicked");
}
