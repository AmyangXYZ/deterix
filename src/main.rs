use rt_nds::{Node, PacketBuilder};
use std::thread;

fn main() {
    let node1 = thread::spawn(move || {
        let mut node = Node::new(1, "localhost:7778", false);
        node.recv(|packet| {
            println!(
                "Received packet: {:?} -  from {:?}",
                packet.ptype(),
                packet.src()
            );
        });
    });
    let node2 = thread::spawn(move || {
        let mut node = Node::new(2, "localhost:7777", false);
        // thread::sleep(std::time::Duration::from_millis(1_000));
        node.send(
            PacketBuilder::new_data(&node.pool, 2, 1, "test", &[1, 2, 3])
                .expect("Failed to create data packet"),
            "127.0.0.1:7778".parse().unwrap(),
        );
        println!("Sent data packet to 1");
    });

    node1.join().unwrap();
    node2.join().unwrap();
    thread::sleep(std::time::Duration::from_millis(1_000));
}
