use deterix::Node;
use std::thread;
use std::time::Duration;

fn main() {
    let node1 = thread::spawn(move || {
        let mut node = Node::new(1, "localhost:7778", true);
        node.routing_table
            .insert(2, "127.0.0.1:7777".parse().unwrap());
        let packet = node
            .recv(Duration::from_secs(1))
            .expect("No packet received");
        println!(
            "Received packet: {:?} -  from node {}",
            packet.ptype(),
            packet.src()
        );
    });

    let node2 = thread::spawn(move || {
        let mut node = Node::new(2, "localhost:7777", true);
        node.routing_table
            .insert(1, "127.0.0.1:7778".parse().unwrap());
        node.send(1, &[1, 2, 3]);
        println!("Sent data packet to node 1");
    });

    node1.join().expect("Node 1 panicked");
    node2.join().expect("Node 2 panicked");
}
