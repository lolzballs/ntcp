extern crate byteorder;
extern crate core;
extern crate libc;

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

    let endpoint = tcp::Endpoint::new(ipv4::Address::default(), 6969);
    let mut socket = socket::ServerSocket::new(endpoint, raw);
    socket.recv();
    //    listen(recvfd);
}
