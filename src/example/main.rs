use deterix::{Node, PayloadView, Schedule, Slot, ANY_NODE, ORCHESTRATOR_ID};
use std::thread;
use std::time::Duration;

fn main() {
    let orchestrator = thread::spawn(move || {
        let mut orchestrator = Node::new_orchestrator(
            "127.0.0.10:7777",
            10_000,
            Schedule {
                slots: [
                    Slot::new_dedicate(0, 1, 2),
                    Slot::new_dedicate(1, 1, 2),
                    Slot::new_dedicate(2, 1, 2),
                    Slot::new_dedicate(3, 1, 2),
                    Slot::new_dedicate(4, 1, 2),
                    Slot::new_dedicate(5, 1, 2),
                    Slot::new_dedicate(6, 1, 2),
                    Slot::new_dedicate(7, 1, 2),
                    Slot::new_shared(8, ANY_NODE, ORCHESTRATOR_ID),
                    Slot::new_shared(9, ORCHESTRATOR_ID, ANY_NODE),
                ],
            },
            false,
        );
        orchestrator.serve();
    });

    let node1 = thread::spawn(move || {
        let mut node = Node::new(1, "127.0.0.11:7777", false);
        node.join("127.0.0.10:7777");
        println!("[Node 1] Joined");
        node.routing_table
            .insert(2, "127.0.0.12:7777".parse().unwrap());

        for i in 0..10 {
            node.send(
                2,
                format!("Hello world - {}", i)
                    .as_bytes()
                    .repeat(50)
                    .as_slice(),
            );
            println!("[Node 1] Sending data: {}", i);
            // thread::sleep(Duration::from_secs(1));
        }
    });

    let node2 = thread::spawn(move || {
        let mut node = Node::new(2, "127.0.0.12:7777", false);
        node.join("127.0.0.10:7777");
        println!("[Node 2] Joined");

        let start = std::time::Instant::now();
        for i in 0..10 {
            if let Some((packet, _)) = node.recv(Duration::from_secs(5)) {
                if let PayloadView::Data(data) = packet.payload() {
                    println!("[Node 2] Received data: {:?} #{}", data.size(), i);
                }
            }
        }
        println!("[Node 2] Total time: {:?}", start.elapsed());
    });

    orchestrator.join().expect("Orchestrator panicked");
    node1.join().expect("Node 1 panicked");
    node2.join().expect("Node 2 panicked");
}
