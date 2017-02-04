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
                let mut ippayload = &mut ip.payload_mut()[..iprepr.payload_len];
                let mut tcp = tcp::Packet::new(ippayload).unwrap();
                tcp.set_src_port(self.endpoint.port);
                tcp.set_dst_port(endpoint.port);
                tcp.set_data_offset(20);

                tcp.set_ack_num(recv.seq_num() + 1);
                tcp.set_seq_num(123123);

                tcp.set_flag_syn(true);
                tcp.set_flag_ack(true);

                tcp.fill_checksum(&src_addr, &endpoint.addr);

                println!("{:?}", tcp);
            }

            let total_len = ip.total_len() as usize;
            total_len
        };

        println!("{:?}", &buf[..len]);
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
                println!("{:?}", ip);
                println!("{:?}", tcp);
                let endpoint = tcp::Endpoint::new(iprepr.src_addr, tcprepr.src_port);

                if tcp.flag_syn() && !tcp.flag_ack() {
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
