extern crate ntcp;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

use ntcp::ipv4;
use ntcp::platform::RawSocket;
use ntcp::socket::{PacketBuffer, SocketInterface, Socket};
use ntcp::tcp;

fn create_client(mut tcp: TcpStream, tx: mpsc::Sender<Vec<u8>>, rx: mpsc::Receiver<Vec<u8>>) {
    let raw = RawSocket::new().unwrap();

    let endpoint = tcp::Endpoint::new(ipv4::Address::from_bytes(&[127, 0, 0, 1]), 8090);
    let mut interface = SocketInterface::new(endpoint, raw);

    let mut socket = interface
        .connect(tcp::Endpoint::new(ipv4::Address::from_bytes(&[127, 0, 0, 1]), 6969))
        .unwrap();

    let endpoint = socket.endpoint;
    let (socket_tx, socket_rx) = socket.to_tx_rx();

    {
        let mut tcp = tcp.try_clone().unwrap();
        thread::spawn(move || loop {
                          let mut buf = [0; 17000];
                          let len = tcp.read(&mut buf).unwrap();
                          if len == 0 {
                              break;
                          }
                          println!("Sent: {:?}", len);
                          socket_tx
                              .send((endpoint, PacketBuffer::new(&buf[..len])))
                              .unwrap();
                      });
    }
    thread::spawn(move || loop {
                      let packet = socket_rx.recv().unwrap();
                      println!("Recieved: {:?}", packet.payload.len());
                      tcp.write_all(&(*packet.payload)).unwrap();
                  });
}

fn main() {
    let socket = TcpListener::bind("127.0.0.1:25566").unwrap();

    for stream in socket.incoming() {
        let stream = stream.unwrap();
        let (send_tx, send_rx) = mpsc::channel();
        let (recv_tx, recv_rx) = mpsc::channel();

        create_client(stream, send_tx, recv_rx);
    }
}
