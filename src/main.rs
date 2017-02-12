extern crate byteorder;
extern crate core;
extern crate libc;

use std::sync::{Arc, mpsc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

mod error;
mod ipv4;
mod tcp;
mod platform;
mod socket;

fn create_client(raw: Arc<platform::RawSocket>) {
    let endpoint = tcp::Endpoint::new(ipv4::Address::default(), 8090);
    let mut interface = socket::SocketInterface::new(endpoint, raw);

    let mut socket =
        interface.connect(tcp::Endpoint::new(ipv4::Address::from_bytes(&[127, 0, 0, 1]), 6969))
            .unwrap();
    println!("{:?}", socket);
    socket.send(socket::PacketBuffer::new(&[69, 4, 20])).unwrap();

    println!("Recieved: {:?}", socket.recv());

    thread::sleep(Duration::from_secs(2));
    socket.send(socket::PacketBuffer::new(&[69, 69, 69, 69])).unwrap();
}

fn create_server(raw: Arc<platform::RawSocket>) -> thread::JoinHandle<()> {
    let endpoint = tcp::Endpoint::new(ipv4::Address::default(), 6969);
    let mut interface = socket::SocketInterface::new(endpoint, raw);
    let (tx, rx) = mpsc::channel();
    interface.listen(tx);

    thread::spawn(move || {
        let running = Arc::new(AtomicBool::new(true));
        println!("Server starting...");

        while running.load(Ordering::Relaxed) {
            let mut socket = match rx.try_recv() {
                Ok(socket) => socket,
                Err(mpsc::TryRecvError::Empty) => continue,
                Err(_) => break,
            };
            println!("Connection established with: {:?}", socket.endpoint);
            let running = running.clone();
            thread::spawn(move || {
                loop {
                    let packet = match socket.recv() {
                        Ok(p) => p,
                        Err(_) => break,
                    };

                    if packet.payload.len() == 4 {
                        running.store(false, Ordering::Relaxed);
                    } else {
                        socket.send(socket::PacketBuffer::new(&[4, 20, 4, 20])).unwrap();
                    }
                    println!("{:?}", packet);
                }
                println!("Connection closed with: {:?}", socket.endpoint);
            });
        }

        println!("Server stopping...");
    })
}

fn main() {
    let raw = match platform::RawSocket::new() {
        Ok(socket) => Arc::new(socket),
        Err(error) => panic!("Error creating RawSocket: {}", error),
    };

    let server = create_server(raw.clone());

    let raw = match platform::RawSocket::new() {
        Ok(socket) => Arc::new(socket),
        Err(error) => panic!("Error creating RawSocket: {}", error),
    };
    create_client(raw.clone());

    server.join();
}
