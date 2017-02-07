extern crate byteorder;
extern crate core;
extern crate libc;

use std::sync::mpsc;
use std::thread;

mod error;
mod ipv4;
mod tcp;
mod platform;
mod socket;

fn main() {
    let raw = match platform::RawSocket::new() {
        Ok(socket) => socket,
        Err(error) => panic!("Error creating RawSocket: {}", error),
    };

    let (tx, rx) = mpsc::channel();
    let endpoint = tcp::Endpoint::new(ipv4::Address::default(), 6969);
    let server = socket::ServerSocket::new(endpoint, raw);
    server.listen(tx);

    loop {
        let mut socket = rx.recv().unwrap();
        thread::spawn(move || {
            socket.send(socket::PacketBuffer::new(&[97, 98])).unwrap();
            loop {
                let packet = match socket.recv() {
                    Ok(p) => p,
                    Err(_) => break,
                };
                println!("{:?}", packet);
            }
        });
    }
}
