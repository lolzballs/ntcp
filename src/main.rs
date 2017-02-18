extern crate byteorder;
extern crate core;
extern crate libc;

use std::io::Read;
use std::sync::{Arc, Mutex, mpsc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

mod error;
mod ipv4;
mod tcp;
mod platform;
mod socket;

fn create_client(raw: platform::RawSocket) {
    let endpoint = tcp::Endpoint::new(ipv4::Address::default(), 8090);
    let mut interface = socket::SocketInterface::new(endpoint, raw);

    let mut socket =
        interface.connect(tcp::Endpoint::new(ipv4::Address::from_bytes(&[127, 0, 0, 1]), 6969))
            .unwrap();
    socket.send(socket::PacketBuffer::new(&[69, 4, 20])).unwrap();

    let mut buf = [0; 1024];
    let len = socket.read(&mut buf).unwrap();
    println!("Client recieved: {:?}", &buf[..len]);

    thread::sleep(Duration::from_secs(2));
    socket.send(socket::PacketBuffer::new(&[69, 69, 69, 69])).unwrap();
}

fn create_server(raw: platform::RawSocket) -> thread::JoinHandle<()> {
    let endpoint = tcp::Endpoint::new(ipv4::Address::default(), 6969);
    let mut interface = socket::SocketInterface::new(endpoint, raw);
    let (tx, rx) = mpsc::channel();
    interface.listen(tx);

    thread::spawn(move || {
        let interface = Arc::new(Mutex::new(interface));
        println!("Server starting...");

        loop {
            let mut socket = match rx.recv() {
                Ok(socket) => socket,
                Err(_) => break,
            };
            let interface = interface.clone();
            println!("Connection established with: {:?}", socket.endpoint);
            thread::spawn(move || {
                let mut buf = [0; 1024];
                loop {
                    let packet = match socket.read(&mut buf) {
                        Ok(len) => &buf[..len],
                        Err(_) => break,
                    };

                    println!("Server recieved: {:?}", packet);
                    if packet.len() == 4 {
                        interface.lock().unwrap().stop();
                    } else {
                        socket.send(socket::PacketBuffer::new(&[4, 20, 4, 20])).unwrap();
                    }
                }
                println!("Connection closed with: {:?}", socket.endpoint);
            });
        }

        println!("Server stopping...");
    })
}

fn main() {
    let raw = match platform::RawSocket::new() {
        Ok(socket) => socket,
        Err(error) => panic!("Error creating RawSocket: {}", error),
    };

    let server = create_server(raw);

    let raw = match platform::RawSocket::new() {
        Ok(socket) => socket,
        Err(error) => panic!("Error creating RawSocket: {}", error),
    };
    create_client(raw);

    server.join().unwrap();
}
