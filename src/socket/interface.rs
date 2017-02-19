use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::mem;
use std::sync::{Arc, Mutex, mpsc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use super::{PacketBuffer, Socket, SocketState};
use ::tcp;
use ::ipv4;
use ::platform;
use ::error::{Error, SocketError};

const RECV_BUF_LEN: usize = 2048;

type SocketMap = HashMap<tcp::Endpoint, (SocketState, Option<mpsc::Sender<PacketBuffer>>)>;

pub struct Interface {
    running: Arc<AtomicBool>,
    endpoint: tcp::Endpoint,
    raw: Arc<platform::RawSocket>,
    sockets: Arc<Mutex<SocketMap>>,

    send_thread: Option<thread::JoinHandle<()>>,
    recv_thread: Option<thread::JoinHandle<()>>,
}

impl Interface {
    pub fn new(endpoint: tcp::Endpoint, raw: platform::RawSocket) -> Self {
        Interface {
            running: Arc::new(AtomicBool::new(false)),
            endpoint: endpoint,
            raw: Arc::new(raw),
            sockets: Arc::new(Mutex::new(HashMap::new())),

            send_thread: None,
            recv_thread: None,
        }
    }

    pub fn connect(&mut self, remote: tcp::Endpoint) -> Result<Socket, SocketError> {
        let (tx, rx) = mpsc::channel::<Socket>();
        if !self.running.load(Ordering::Relaxed) {
            self.start(tx);
        }
        {
            let mut sockets = self.sockets.lock().unwrap();
            sockets.insert(remote, (SocketState::SynSent, None));
            Self::send_syn(&self.raw, self.endpoint, remote);
        }
        rx.recv_timeout(Duration::from_secs(2)).map_err(|_| SocketError::Timeout)
    }

    pub fn close(&mut self, remote: tcp::Endpoint) {
        let mut sockets = self.sockets.lock().unwrap();
        sockets.remove(&remote);
    }

    pub fn listen(&mut self, tx: mpsc::Sender<Socket>) {
        self.start(tx);
    }

    pub fn start(&mut self, tx: mpsc::Sender<Socket>) {
        self.running.store(true, Ordering::Relaxed);
        let (tx_send, tx_recv) = mpsc::channel::<(tcp::Endpoint, PacketBuffer)>();

        self.send_thread = Some({
            let running = self.running.clone();
            let local = self.endpoint;
            let raw = self.raw.clone();
            let mut sockets = self.sockets.clone();
            thread::spawn(move || {
                while running.load(Ordering::Relaxed) {
                    let buf = tx_recv.recv().unwrap();
                    Self::send(&raw, &mut sockets, local, buf.0, &*buf.1.payload);
                }
            })
        });

        self.recv_thread = Some({
            let running = self.running.clone();
            let endpoint = self.endpoint;
            let raw = self.raw.clone();
            let sockets = self.sockets.clone();
            thread::spawn(move || {
                Self::recv(raw, running, endpoint, sockets, tx, tx_send);
            })
        });
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);

        if let Some(handle) = mem::replace(&mut self.send_thread, None) {
            handle.join().unwrap();
        }

        if let Some(handle) = mem::replace(&mut self.recv_thread, None) {
            handle.join().unwrap();
        }
    }

    fn send_syn(raw: &Arc<platform::RawSocket>, local: tcp::Endpoint, remote: tcp::Endpoint) {
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

    fn send_ack(raw: &Arc<platform::RawSocket>,
                recv: &tcp::Packet<&[u8]>,
                local: tcp::Endpoint,
                remote: tcp::Endpoint) {
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
                    seq: recv.ack_num() + 1,
                    ack: Some(recv.seq_num() + 1),
                    control: tcp::Control::None,
                    payload: &[],
                };

                tcprepr.emit(&mut tcp, &local.addr, &remote.addr);
            }

            let total_len = ip.total_len() as usize;
            total_len
        };

        raw.send(remote, &buf[..len]).unwrap();
    }

    fn send_syn_ack(raw: &Arc<platform::RawSocket>,
                    recv: &tcp::Packet<&[u8]>,
                    local: tcp::Endpoint,
                    remote: tcp::Endpoint) {
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

    fn send(raw: &Arc<platform::RawSocket>,
            sockets: &Arc<Mutex<SocketMap>>,
            local: tcp::Endpoint,
            remote: tcp::Endpoint,
            payload: &[u8]) {
        let mut buf = vec![0; 40 + payload.len()];
        let len = {
            let iprepr = ipv4::Repr {
                src_addr: local.addr,
                dst_addr: remote.addr,
                payload_len: 20 + payload.len(),
            };
            let mut ip = ipv4::Packet::new(&mut buf[..]).unwrap();
            {
                iprepr.send(&mut ip);
            }
            {
                let mut tcp = tcp::Packet::new(&mut ip.payload_mut()[..iprepr.payload_len])
                    .unwrap();

                let (seq, ack) = {
                    let mut sockets = sockets.lock().unwrap();
                    if let SocketState::Established { seq: ref mut seq, ack: ref mut ack } =
                        sockets.get_mut(&remote).unwrap().0 {
                        let seqack = (*seq, *ack);
                        println!("SEQ: {}, ACK: {:?}", seq, ack);
                        *seq += payload.len() as u32;
                        seqack
                    } else {
                        return;
                    }
                };

                let tcprepr = tcp::Repr {
                    src_port: local.port,
                    dst_port: remote.port,
                    seq: seq,
                    ack: Some(ack),
                    control: tcp::Control::None,
                    payload: payload,
                };

                tcprepr.emit(&mut tcp, &local.addr, &remote.addr);
            }

            let total_len = ip.total_len() as usize;
            total_len
        };

        raw.send(remote, &buf[..len]).unwrap();
    }

    fn process_tcp(raw: &Arc<platform::RawSocket>,
                   local: tcp::Endpoint,
                   remote: tcp::Endpoint,
                   tcp: tcp::Packet<&[u8]>,
                   tcprepr: tcp::Repr,
                   sockets: &Arc<Mutex<SocketMap>>,
                   socket_send: &mpsc::Sender<Socket>,
                   tx_send: &mpsc::Sender<(tcp::Endpoint, PacketBuffer)>) {
        let mut sockets = sockets.lock().unwrap();
        {
            if let Entry::Occupied(mut socket_entry) = sockets.entry(remote) {
                match tcprepr.control {
                    tcp::Control::Rst => {
                        socket_entry.remove_entry();
                    }
                    tcp::Control::Syn => {
                        let mut socket = socket_entry.get_mut();
                        match socket.0 {
                            // SYN-ACK of handshake
                            SocketState::SynSent => {
                                if tcprepr.ack.is_some() {
                                    Self::send_ack(&raw, &tcp, local, remote);
                                    let (rx_tx, rx_rx) = mpsc::channel();


                                    socket_send.send(Socket::new(remote, rx_rx, tx_send.clone()))
                                        .unwrap();
                                    socket.0 = SocketState::Established {
                                        seq: tcprepr.ack.unwrap(),
                                        ack: tcprepr.seq,
                                    };
                                    println!("SEQ: {}, ACK: {:?}",
                                             tcprepr.ack.unwrap(),
                                             tcprepr.seq);
                                    socket.1 = Some(rx_tx);
                                }
                            }
                            _ => (),
                        }
                    }
                    tcp::Control::None => {
                        let mut socket = socket_entry.get_mut();
                        match socket.0 {
                            SocketState::SynSent => (),
                            // ACK in response to SYN-ACK
                            SocketState::SynReceived => {
                                if tcprepr.ack.is_some() {
                                    socket.0 = SocketState::Established {
                                        seq: tcprepr.ack.unwrap(),
                                        ack: tcprepr.seq,
                                    };
                                    println!("SEQ: {}, ACK: {:?}",
                                             tcprepr.ack.unwrap(),
                                             tcprepr.seq);
                                }
                            }
                            SocketState::Established { ack: ref mut ack, seq: seq } => {
                                let socket = match socket.1 {
                                    Some(ref socket) => socket,
                                    None => return,
                                };
                                *ack += tcp.payload().len() as u32;
                                socket.send(PacketBuffer::new(tcp.payload()))
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
            }
        }

        // Initial SYN in handshake
        if tcprepr.control == tcp::Control::Syn {
            if tcprepr.ack.is_none() {
                Self::send_syn_ack(&raw, &tcp, local, remote);

                // Channel for sending packets
                let (rx_tx, rx_rx) = mpsc::channel();

                socket_send.send(Socket::new(remote, rx_rx, tx_send.clone()))
                    .unwrap();
                sockets.insert(remote, (SocketState::SynReceived, Some(rx_tx)));
            }
        }
    }

    fn recv(raw: Arc<platform::RawSocket>,
            running: Arc<AtomicBool>,
            local: tcp::Endpoint,
            sockets: Arc<Mutex<SocketMap>>,
            socket_send: mpsc::Sender<Socket>,
            tx_send: mpsc::Sender<(tcp::Endpoint, PacketBuffer)>) {
        while running.load(Ordering::Relaxed) {
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
                let local = tcp::Endpoint::new(iprepr.dst_addr, tcprepr.dst_port);
                let remote = tcp::Endpoint::new(iprepr.src_addr, tcprepr.src_port);
                Self::process_tcp(&raw,
                                  local,
                                  remote,
                                  tcp,
                                  tcprepr,
                                  &sockets,
                                  &socket_send,
                                  &tx_send);
            }
        }
    }
}
