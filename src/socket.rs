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

    fn send_syn_ack(&self, recv: &tcp::Packet<&[u8]>, endpoint: tcp::Endpoint) {
        let mut buf = [0; 50];
        let src_addr = ipv4::Address::from_bytes(&[127, 0, 0, 1]);
        let len = {
            let iprepr = ipv4::Repr {
                src_addr: ipv4::Address::from_bytes(&[127, 0, 0, 1]),
                dst_addr: endpoint.addr,
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
                    src_port: self.endpoint.port,
                    dst_port: endpoint.port,
                    seq: 123123,
                    ack: Some(recv.seq_num() + 1),
                    control: tcp::Control::Syn,
                    payload: &[],
                };

                tcprepr.emit(&mut tcp, &src_addr, &endpoint.addr);

                println!("{:?}", tcprepr);
                println!("{:?}", tcp);
            }

            let total_len = ip.total_len() as usize;
            total_len
        };

        self.raw.send(endpoint, &buf[..len]).unwrap();
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

                if tcprepr.control == tcp::Control::Syn && tcprepr.ack.is_none() {
                    println!("HANDSHAKE");
                    self.send_syn_ack(&tcp, endpoint);
                } else {
                    return Ok(PacketBuffer::new(endpoint, tcp.payload()));
                }
            }
        }
    }

    pub fn send(&self, buf: PacketBuffer) {
        self.raw.send(buf.endpoint, &*buf.payload).unwrap();
    }
}
