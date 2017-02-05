use super::ipv4;
use super::tcp;
use super::platform;

use std::io;

use error::Error;

const RECV_BUF_LEN: usize = 2048;

#[derive(Debug)]
pub struct PacketBuffer {
    endpoint: tcp::Endpoint,
    payload: Box<[u8]>,
}

impl PacketBuffer {
    pub fn new(endpoint: tcp::Endpoint, payload: &[u8]) -> Self {
        PacketBuffer {
            endpoint: endpoint,
            payload: payload.to_vec().into_boxed_slice(),
        }
    }
}

pub struct Socket {
    endpoint: tcp::Endpoint,
    raw: platform::RawSocket,
}

impl Socket {
    pub fn new(endpoint: tcp::Endpoint, raw: platform::RawSocket) -> Self {
        Socket {
            endpoint: endpoint,
            raw: raw,
        }
    }

    fn send_syn_ack(&self, recv: &tcp::Packet<&[u8]>, src: tcp::Endpoint, dst: tcp::Endpoint) {
        let mut buf = [0; 50];
        let src_addr = src.addr;
        let len = {
            let iprepr = ipv4::Repr {
                src_addr: src.addr,
                dst_addr: dst.addr,
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
                    src_port: src.port,
                    dst_port: dst.port,
                    seq: 123123,
                    ack: Some(recv.seq_num() + 1),
                    control: tcp::Control::Syn,
                    payload: &[],
                };

                tcprepr.emit(&mut tcp, &src.addr, &dst.addr);
            }

            let total_len = ip.total_len() as usize;
            total_len
        };

        self.raw.send(dst, &buf[..len]).unwrap();
    }

    pub fn recv(&self) -> io::Result<PacketBuffer> {
        loop {
            let mut buf = [0; RECV_BUF_LEN];
            let len = try!(self.raw.recv(&mut buf));
            let ip = ipv4::Packet::new(&buf[..len]).unwrap();
            let iprepr = match ipv4::Repr::parse(&ip) {
                Ok(repr) => repr,
                Err(Error::UnknownProtocol) => continue,
                Err(Error::Truncated) => {
                    println!("WARN: IPv4 packet exceeded MTU");
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
            if tcprepr.dst_port == self.endpoint.port {
                let endpoint = tcp::Endpoint::new(iprepr.src_addr, tcprepr.src_port);

                match tcprepr.control {
                    tcp::Control::Syn => {
                        if tcprepr.ack.is_none() {
                            self.send_syn_ack(&tcp,
                                              tcp::Endpoint::new(iprepr.dst_addr,
                                                                 tcprepr.dst_port),
                                              endpoint);
                        }
                    }
                    tcp::Control::None => {
                        return Ok(PacketBuffer::new(endpoint, tcp.payload()));
                    }
                    _ => println!("{:?}", tcp),
                }
            }
        }
    }

    pub fn send(&self, buf: PacketBuffer) {
        self.raw.send(buf.endpoint, &*buf.payload).unwrap();
    }
}
