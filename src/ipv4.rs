use byteorder::{ByteOrder, NetworkEndian};
use super::error::Error;
use std::fmt;

const TCP_PROTOCOL: u8 = 6;

#[derive(Debug, Default)]
pub struct Address([u8; 4]);

impl Address {
    pub fn from_bytes(data: &[u8]) -> Self {
        let mut bytes = [0; 4];
        bytes.copy_from_slice(data);
        Address(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn as_beu32(&self) -> u32 {
        NetworkEndian::read_u32(&self.0)
    }
}

pub struct Packet<'a> {
    buffer: &'a [u8],
}

mod field {
    type Field = ::core::ops::Range<usize>;

    pub const VER_IHL: usize = 0;
    pub const DSCP_ECN: usize = 1;
    pub const LENGTH: Field = 2..4;
    pub const ID: Field = 4..6;
    pub const FLG_OFF: Field = 6..8;
    pub const TTL: usize = 8;
    pub const PROTOCOL: usize = 9;
    pub const CHECKSUM: Field = 10..12;
    pub const SRC_ADDR: Field = 12..16;
    pub const DST_ADDR: Field = 16..20;
}

impl<'a> Packet<'a> {
    pub fn new(buffer: &'a [u8]) -> Result<Self, Error> {
        let len = buffer.len();
        if len < field::DST_ADDR.end {
            Err(Error::Truncated)
        } else {
            let packet = Packet { buffer: buffer };
            if len < packet.header_len() as usize {
                Err(Error::Truncated)
            } else {
                Ok(packet)
            }
        }
    }

    #[inline]
    pub fn version(&self) -> u8 {
        let buf = self.buffer.as_ref();
        buf[field::VER_IHL] >> 4
    }

    #[inline]
    pub fn header_len(&self) -> u8 {
        let buf = self.buffer.as_ref();
        (buf[field::VER_IHL] & 0x0F) * 4
    }

    #[inline]
    pub fn dscp(&self) -> u8 {
        let buf = self.buffer.as_ref();
        buf[field::VER_IHL] >> 2
    }

    #[inline]
    pub fn ecn(&self) -> u8 {
        let buf = self.buffer.as_ref();
        buf[field::VER_IHL] & 0b11
    }

    #[inline]
    pub fn total_len(&self) -> u16 {
        let buf = self.buffer.as_ref();
        NetworkEndian::read_u16(&buf[field::LENGTH])
    }

    #[inline]
    pub fn identification(&self) -> u16 {
        let buf = self.buffer.as_ref();
        NetworkEndian::read_u16(&buf[field::ID])
    }

    /// Don't Fragment (DF) flag
    #[inline]
    pub fn flag_df(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::FLG_OFF.start] & 0x40 != 0
    }

    /// More Fragments (MF) flag
    #[inline]
    pub fn flag_mf(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::FLG_OFF.start] & 0x20 != 0
    }

    #[inline]
    pub fn fragment_offset(&self) -> u16 {
        let buf = self.buffer.as_ref();
        NetworkEndian::read_u16(&buf[field::FLG_OFF]) << 3
    }

    #[inline]
    pub fn ttl(&self) -> u8 {
        let buf = self.buffer.as_ref();
        buf[field::TTL]
    }

    #[inline]
    pub fn protocol(&self) -> u8 {
        let buf = self.buffer.as_ref();
        buf[field::PROTOCOL]
    }

    #[inline]
    pub fn checksum(&self) -> u16 {
        let buf = self.buffer.as_ref();
        NetworkEndian::read_u16(&buf[field::CHECKSUM])
    }

    #[inline]
    pub fn src_addr(&self) -> Address {
        let buf = self.buffer.as_ref();
        Address::from_bytes(&buf[field::SRC_ADDR])
    }

    #[inline]
    pub fn dst_addr(&self) -> Address {
        let buf = self.buffer.as_ref();
        Address::from_bytes(&buf[field::DST_ADDR])
    }

    #[inline]
    pub fn payload(&self) -> &[u8] {
        let buf = self.buffer.as_ref();
        &buf[self.header_len() as usize..]
    }
}

impl<'a> fmt::Debug for Packet<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Ipv4Packet")
            .field("dscp", &self.dscp())
            .field("ecn", &self.ecn())
            .field("identification", &self.identification())
            .field("df", &self.flag_df())
            .field("mf", &self.flag_mf())
            .field("ttl", &self.ttl())
            .field("protocol", &self.protocol())
            .field("checksum", &self.checksum())
            .field("src_addr", &self.src_addr())
            .field("dst_addr", &self.dst_addr())
            .finish()
    }
}

#[derive(Debug)]
pub struct Repr {
    pub src_addr: Address,
    pub dst_addr: Address,
    pub payload_len: usize,
}

impl Repr {
    pub fn parse(packet: &Packet) -> Result<Self, Error> {
        if packet.version() != 4 {
            return Err(Error::Malformed);
        }

        if packet.header_len() > 20 {
            return Err(Error::Unrecognized);
        }

        if packet.flag_mf() || packet.fragment_offset() != 0 {
            return Err(Error::Fragmented);
        }

        if packet.protocol() != TCP_PROTOCOL {
            return Err(Error::UnknownProtocol);
        }

        let payload_len = packet.total_len() as usize - packet.header_len() as usize;
        if packet.payload().len() < payload_len {
            return Err(Error::Truncated);
        }

        Ok(Repr {
            src_addr: packet.src_addr(),
            dst_addr: packet.dst_addr(),
            payload_len: payload_len,
        })
    }
}
