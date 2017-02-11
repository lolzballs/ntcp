use super::ipv4;
use super::tcp;
use super::platform;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use error::{Error, SocketError};

const RECV_BUF_LEN: usize = 2048;

#[derive(Debug)]
pub struct PacketBuffer {
    payload: Box<[u8]>,
}

impl PacketBuffer {
    pub fn new(payload: &[u8]) -> Self {
        PacketBuffer { payload: payload.to_vec().into_boxed_slice() }
    }
}

#[derive(Debug, PartialEq)]
enum SocketState {
    SynSent,
    SynReceived,
    Established,
    Closed,
}

pub struct SocketInterface {
    listening: bool,
    endpoint: tcp::Endpoint,
    raw: Arc<platform::RawSocket>,
    sockets: Arc<Mutex<HashMap<tcp::Endpoint, (SocketState, mpsc::Sender<PacketBuffer>)>>>,
}

impl SocketInterface {
    pub fn new(endpoint: tcp::Endpoint, raw: Arc<platform::RawSocket>) -> Self {
        SocketInterface {
            listening: false,
            endpoint: endpoint,
            raw: raw,
            sockets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create(&mut self, remote: tcp::Endpoint) -> Result<Socket, SocketError> {
        let (tx, rx) = mpsc::channel::<Socket>();
        if !self.listening {
            self.start(tx);
        }
        Self::send_syn(self.raw.clone(), self.endpoint, remote);
        rx.recv_timeout(Duration::from_secs(2)).map_err(|_| SocketError::Timeout)
    }

    pub fn listen(&mut self, tx: mpsc::Sender<Socket>) {
        self.start(tx);
    }

    pub fn start(&mut self, tx: mpsc::Sender<Socket>) {
        self.listening = true;
        let (tx_send, tx_recv) = mpsc::channel::<(tcp::Endpoint, PacketBuffer)>();
        {
            let raw = self.raw.clone();
            thread::spawn(move || {
                loop {
                    let buf = tx_recv.recv().unwrap();
                    println!("TODO: send {:?}", buf);
                    raw.send(buf.0, &*buf.1.payload).unwrap();
                }
            });
        }
        {
            let endpoint = self.endpoint;
            let raw = self.raw.clone();
            let sockets = self.sockets.clone();
            thread::spawn(move || {
                Self::recv(endpoint, raw, sockets, tx, tx_send);
            });
        }
    }

    fn send_syn(raw: Arc<platform::RawSocket>, local: tcp::Endpoint, remote: tcp::Endpoint) {
        let mut buf = [0; 40];
        let len = {
            let iprepr = ipv4::Repr {
                src_addr: local.addr,
                dst_addr: remote.addr,
                payload_len: 20,
            };
            let mut ip = ipv4::Packet::new(&mut buf[..]).unwrap();
            {
                iprepr.send(&mut ip);
            }
            {
                let mut tcp = tcp::Packet::new(&mut ip.payload_mut()[..iprepr.payload_len])
                    .unwrap();

                let tcprepr = tcp::Repr {
                    src_port: local.port,
                    dst_port: remote.port,
                    seq: 123123,
                    ack: None,
                    control: tcp::Control::Syn,
                    payload: &[],
                };

                tcprepr.emit(&mut tcp, &local.addr, &remote.addr);
            }

            let total_len = ip.total_len() as usize;
            total_len
        };

        raw.send(remote, &buf[..len]).unwrap();
    }

    fn send_syn_ack(raw: Arc<platform::RawSocket>,
                    recv: &tcp::Packet<&[u8]>,
                    local: tcp::Endpoint,
                    remote: tcp::Endpoint) {
        let mut buf = [0; 50];
        let len = {
            let iprepr = ipv4::Repr {
                src_addr: local.addr,
                dst_addr: remote.addr,
                payload_len: 24,
            };
            let mut ip = ipv4::Packet::new(&mut buf[..]).unwrap();
            {
                iprepr.send(&mut ip);
            }
            {
                let mut tcp = tcp::Packet::new(&mut ip.payload_mut()[..iprepr.payload_len])
                    .unwrap();

                let tcprepr = tcp::Repr {
                    src_port: local.port,
                    dst_port: remote.port,
                    seq: 123123,
                    ack: Some(recv.seq_num() + 1),
                    control: tcp::Control::Syn,
                    payload: &[],
                };

                tcprepr.emit(&mut tcp, &local.addr, &remote.addr);
            }

            let total_len = ip.total_len() as usize;
            total_len
        };

        raw.send(remote, &buf[..len]).unwrap();
    }

    fn recv(local: tcp::Endpoint,
            raw: Arc<platform::RawSocket>,
            sockets: Arc<Mutex<HashMap<tcp::Endpoint,
                                       (SocketState, mpsc::Sender<PacketBuffer>)>>>,
            socket_send: mpsc::Sender<Socket>,
            tx_send: mpsc::Sender<(tcp::Endpoint, PacketBuffer)>) {
        loop {
            let mut buf = [0; RECV_BUF_LEN];
            let len = raw.recv(&mut buf).unwrap_or(0);
            if len == 0 {
                continue;
            }
            let ip = ipv4::Packet::new(&buf[..len]).unwrap();
            let iprepr = match ipv4::Repr::parse(&ip) {
                Ok(repr) => repr,
                Err(Error::UnknownProtocol) => continue,
                Err(Error::Truncated) => {
                    // println!("WARN: IPv4 packet exceeded MTU");
                    continue;
                }
                Err(error) => {
                    println!("WARN: IPv4 packet {:?}", error);
                    continue;
                }
            };
            let tcp = tcp::Packet::new(&ip.payload()[..iprepr.payload_len]).unwrap();
            let tcprepr = match tcp::Repr::parse(&tcp, &iprepr.src_addr, &iprepr.dst_addr) {
                Ok(repr) => repr,
                Err(error) => {
                    println!("WARN: TCP packet {:?}", error);
                    continue;
                }
            };
            if tcprepr.dst_port == local.port {
                let endpoint = tcp::Endpoint::new(iprepr.src_addr, tcprepr.src_port);
                let mut sockets = sockets.lock().unwrap();
                let known = {
                    if let Entry::Occupied(mut socket_entry) = sockets.entry(endpoint) {
                        match tcprepr.control {
                            tcp::Control::Rst => {
                                socket_entry.remove_entry();
                            }
                            tcp::Control::None => {
                                let mut socket = socket_entry.get_mut();
                                match socket.0 {
                                    SocketState::SynSent => (),
                                    SocketState::SynReceived => {
                                        if tcprepr.ack.is_some() {
                                            socket.0 = SocketState::Established;
                                        }
                                    }
                                    SocketState::Established => {
                                        (socket.1)
                                            .send(PacketBuffer::new(tcp.payload()))
                                            .unwrap();
                                    }
                                    SocketState::Closed => (),
                                };
                            }
                            _ => {
                                println!("WARNING: Control flag not implemented({:?})",
                                         tcprepr.control)
                            }
                        }
                        true
                    } else {
                        false
                    }
                };
                if !known {
                    if tcprepr.control == tcp::Control::Syn {
                        if tcprepr.ack.is_none() {

                            Self::send_syn_ack(raw.clone(),
                                               &tcp,
                                               tcp::Endpoint::new(iprepr.dst_addr,
                                                                  tcprepr.dst_port),
                                               endpoint);
                            // Channel for sending packets
                            let (rx_tx, rx_rx) = mpsc::channel();


                            socket_send.send(Socket::new(endpoint, rx_rx, tx_send.clone()))
                                .unwrap();
                            sockets.insert(endpoint, (SocketState::SynReceived, rx_tx));
                        }
                    }
                }
            }
        }
    }
}

pub struct ServerSocket {
    interface: SocketInterface,
    tx_socket: mpsc::Sender<Socket>,
}

impl ServerSocket {
    pub fn new(interface: SocketInterface, tx_socket: mpsc::Sender<Socket>) -> Self {
        ServerSocket {
            interface: interface,
            tx_socket: tx_socket,
        }
    }

    pub fn listen(mut self) {
        self.interface.listen(self.tx_socket);
    }
}

pub struct Socket {
    pub endpoint: tcp::Endpoint,
    rx: mpsc::Receiver<PacketBuffer>,
    tx: mpsc::Sender<(tcp::Endpoint, PacketBuffer)>,
}

impl Socket {
    pub fn new(endpoint: tcp::Endpoint,
               rx: mpsc::Receiver<PacketBuffer>,
               tx: mpsc::Sender<(tcp::Endpoint, PacketBuffer)>)
               -> Self {
        Socket {
            endpoint: endpoint,
            rx: rx,
            tx: tx,
        }
    }

    pub fn recv(&mut self) -> Result<PacketBuffer, SocketError> {
        self.rx.recv().map_err(|_| SocketError::Closed)
    }

    pub fn send(&mut self, buf: PacketBuffer) -> Result<(), SocketError> {
        self.tx.send((self.endpoint, buf)).map_err(|_| SocketError::Closed)
    }
}
