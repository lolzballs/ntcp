extern crate ntcp;

use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::mpsc;
use std::thread;

use ntcp::ipv4;
use ntcp::platform::RawSocket;
use ntcp::socket::{PacketBuffer, SocketInterface, Socket};
use ntcp::tcp;

fn create_client(tx: mpsc::Sender<Vec<u8>>, rx: mpsc::Receiver<Vec<u8>>) {
    let raw = RawSocket::new().unwrap();

    let endpoint = tcp::Endpoint::new(ipv4::Address::from_bytes(&[127, 0, 0, 1]), 8090);
    let mut interface = SocketInterface::new(endpoint, raw);

    let mut socket = interface
        .connect(tcp::Endpoint::new(ipv4::Address::from_bytes(&[127, 0, 0, 1]), 6969))
        .unwrap();

    let endpoint = socket.endpoint;
    let (socket_tx, socket_rx) = socket.to_tx_rx();

    thread::spawn(move || loop {
                      let packet = rx.recv().unwrap();
                      println!("Sent: {:?}", packet.len());
                      socket_tx
                          .send((endpoint, PacketBuffer::new(&packet)))
                          .unwrap();
                  });
    thread::spawn(move || loop {
                      let packet = socket_rx.recv().unwrap();
                      println!("Recieved: {:?}", packet.payload.len());
                      tx.send((*packet.payload).to_vec()).unwrap();
                  });
}

fn main() {
    let socket = UdpSocket::bind("127.0.0.1:25566").unwrap();

    let mut sockets = HashMap::new();
    loop {
        let mut buf = [0; 17000];
        let (len, src) = socket.recv_from(&mut buf).unwrap();

        println!("{}", len);
        let tx = sockets
            .entry(src)
            .or_insert_with(|| {
                let (send_tx, send_rx) = mpsc::channel();
                let (recv_tx, recv_rx) = mpsc::channel();

                create_client(send_tx, recv_rx);

                {
                    let socket = socket.try_clone().unwrap();
                    thread::spawn(move || loop {
                                      let packet = send_rx.recv().unwrap();
                                      socket.send_to(&packet, src).unwrap();
                                  });
                }
                recv_tx
            });

        tx.send(buf[..len].to_vec()).unwrap();
    }
}
