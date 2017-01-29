use byteorder::{ByteOrder, NetworkEndian};
use super::error::Error;
use super::ipv4;
use std::fmt;

#[derive(Debug, Default)]
pub struct Endpoint {
    pub addr: ipv4::Address,
    pub port: u16,
}

impl Endpoint {
    pub fn new(addr: ipv4::Address, port: u16) -> Self {
        Endpoint {
            addr: addr,
            port: port,
        }
    }
}

pub struct Packet<'a> {
    buffer: &'a [u8],
}

mod field {
    type Field = ::core::ops::Range<usize>;

    pub const SRC_PORT: Field = 0..2;
    pub const DST_PORT: Field = 2..4;
    pub const SEQ_NUM: Field = 4..8;
    pub const ACK_NUM: Field = 8..12;
    pub const OFF_FLG: Field = 12..14;
    pub const WINDOW_SIZE: Field = 14..16;
    pub const CHECKSUM: Field = 16..18;
    pub const URGENT: Field = 18..20;
}

impl<'a> Packet<'a> {
    pub fn new(buffer: &'a [u8]) -> Result<Self, Error> {
        let len = buffer.len();
        if len < field::URGENT.end {
            Err(Error::Truncated)
        } else {
            let packet = Packet { buffer: buffer };
            if len < packet.data_offset() as usize {
                Err(Error::Truncated)
            } else {
                Ok(packet)
            }
        }
    }

    #[inline]
    pub fn src_port(&self) -> u16 {
        let buf = self.buffer.as_ref();
        NetworkEndian::read_u16(&buf[field::SRC_PORT])
    }

    #[inline]
    pub fn dst_port(&self) -> u16 {
        let buf = self.buffer.as_ref();
        NetworkEndian::read_u16(&buf[field::DST_PORT])
    }

    #[inline]
    pub fn seq_num(&self) -> u32 {
        let buf = self.buffer.as_ref();
        NetworkEndian::read_u32(&buf[field::SEQ_NUM])
    }

    #[inline]
    pub fn ack_num(&self) -> u32 {
        let buf = self.buffer.as_ref();
        NetworkEndian::read_u32(&buf[field::ACK_NUM])
    }

    #[inline]
    pub fn data_offset(&self) -> u8 {
        let buf = self.buffer.as_ref();
        (buf[field::OFF_FLG.start] >> 4) * 4
    }

    #[inline]
    pub fn flag_ns(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.start] & 0x1 != 0
    }

    #[inline]
    pub fn flag_cwr(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end] & 0x80 != 0
    }

    #[inline]
    pub fn flag_ece(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end] & 0x40 != 0
    }

    #[inline]
    pub fn flag_urg(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end] & 0x20 != 0
    }

    #[inline]
    pub fn flag_ack(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end] & 0x10 != 0
    }

    #[inline]
    pub fn flag_psh(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end] & 0x08 != 0
    }

    #[inline]
    pub fn flag_rst(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end] & 0x04 != 0
    }

    #[inline]
    pub fn flag_syn(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end] & 0x02 != 0
    }

    #[inline]
    pub fn flag_fin(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end] & 0x01 != 0
    }

    pub fn payload(&self) -> &[u8] {
        let len = (self.data_offset()) as usize;
        let buf = self.buffer.as_ref();
        &buf[len..]
    }
}

impl<'a> fmt::Debug for Packet<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("TcpPacket")
            .field("src_port", &self.src_port())
            .field("dst_port", &self.dst_port())
            .field("seq_num", &self.seq_num())
            .field("ack_num", &self.ack_num())
            .field("ns", &self.flag_ns())
            .field("cwr", &self.flag_cwr())
            .field("ece", &self.flag_ece())
            .field("urg", &self.flag_urg())
            .field("ack", &self.flag_ack())
            .field("psh", &self.flag_psh())
            .field("rst", &self.flag_rst())
            .field("syn", &self.flag_syn())
            .field("fin", &self.flag_fin())
            .finish()
    }
}

#[derive(Debug)]
pub struct Repr<'a> {
    pub src_port: u16,
    pub dst_port: u16,
    pub payload: &'a [u8],
}

impl<'a> Repr<'a> {
    pub fn parse(packet: &'a Packet,
                 src_addr: &ipv4::Address,
                 dst_addr: &ipv4::Address)
                 -> Result<Self, Error> {
        if packet.src_port() == 0 {
            return Err(Error::Malformed);
        }
        if packet.dst_port() == 0 {
            return Err(Error::Malformed);
        }

        let src_port = packet.src_port();
        let dst_port = packet.dst_port();

        Ok(Repr {
            src_port: src_port,
            dst_port: dst_port,
            payload: packet.payload(),
        })
    }
}
