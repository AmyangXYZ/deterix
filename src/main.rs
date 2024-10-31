use deterix::Node;
use deterix::PayloadView;
use std::thread;
use std::time::Duration;
fn main() {
    let node1 = thread::spawn(move || {
        let mut node = Node::new(1, "localhost:7778", false);
        node.routing_table
            .insert(2, "127.0.0.1:7777".parse().unwrap());
        let packet = node
            .recv(Duration::from_secs(3))
            .expect("No packet received");
        if let PayloadView::Data(data) = packet.payload() {
            println!(
                "Received packet: {:?} ({:?} bytes) from node {}",
                packet.ptype(),
                data.size(),
                packet.src()
            );
        }
    });

    let node2 = thread::spawn(move || {
        let mut node = Node::new(2, "localhost:7777", false);
        node.routing_table
            .insert(1, "127.0.0.1:7778".parse().unwrap());
        let b = [1].repeat(1024);
        node.send(1, &b);
        println!("Enqueued {:?} bytes data sending to node 1", b.len());
    });

    node1.join().expect("Node 1 panicked");
    node2.join().expect("Node 2 panicked");
}
