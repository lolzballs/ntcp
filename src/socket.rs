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
            let tcp = tcp::Packet::new(&ip.payload()).unwrap();
            let tcprepr = match tcp::Repr::parse(&tcp, &iprepr.src_addr, &iprepr.dst_addr) {
                Ok(repr) => repr,
                Err(error) => {
                    println!("WARN: TCP packet {:?}", error);
                    continue;
                }
            };
            if tcprepr.dst_port == self.endpoint.port {
                println!("{:?}", ip);
                println!("{:?}", tcp);
                return Ok(PacketBuffer::new(tcp::Endpoint::new(iprepr.src_addr, tcprepr.src_port),
                                            tcp.payload()));
            }
        }
    }

    pub fn send(&self, buf: PacketBuffer) {
        self.raw.send(buf.endpoint, &*buf.payload).unwrap();
    }
}
