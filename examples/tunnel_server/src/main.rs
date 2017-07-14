extern crate ntcp;

use std::io;
use std::io::{Read, Write};
use std::net::UdpSocket;
use std::sync::mpsc;
use std::thread;

use ntcp::ipv4;
use ntcp::platform::RawSocket;
use ntcp::socket::{PacketBuffer, SocketInterface, Socket};
use ntcp::tcp;

fn create_server() -> thread::JoinHandle<()> {
    let raw = RawSocket::new().unwrap();
    let endpoint = tcp::Endpoint::new(ipv4::Address::default(), 6969);
    let mut interface = SocketInterface::new(endpoint, raw);
    let (tx, rx) = mpsc::channel();
    interface.listen(tx);

    thread::spawn(move || {
        println!("Server starting...");

        loop {
            let mut socket = match rx.recv() {
                Ok(socket) => socket,
                Err(_) => break,
            };
            println!("Connection established with: {:?}", socket.endpoint);
            let endpoint = socket.endpoint;
            let (tx, rx) = socket.to_tx_rx();

            let mut udp = UdpSocket::bind("127.0.0.1:0").unwrap();
            udp.connect("127.0.0.1:25565").unwrap();
            {
                let mut udp = udp.try_clone().unwrap();
                thread::spawn(move || loop {
                                  let mut buf = match rx.recv() {
                                      Ok(buf) => buf,
                                      Err(_) => break,
                                  };

                                  println!("Server recieved: {:?}", buf.payload.len());
                                  udp.send(&*buf.payload);
                              });
            }
            {
                let mut udp = udp.try_clone().unwrap();
                thread::spawn(move || {
                                  let mut buf = [0; 1024];
                                  loop {
                                      let len = udp.recv(&mut buf).unwrap();
                                      println!("Server sent: {:?}", len);
                                      tx.send((endpoint, PacketBuffer::new(&buf[..len])));
                                  }
                              });
            }
        }
    })
}

fn main() {
    let server = create_server();
    server.join();
}
