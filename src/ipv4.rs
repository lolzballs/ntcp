use super::error::Error;

use std::fmt;

use byteorder::{ByteOrder, NetworkEndian};

const TCP_PROTOCOL: u8 = 6;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
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

    pub fn as_u32(&self) -> u32 {
        NetworkEndian::read_u32(&self.0)
    }
}

pub struct Packet<T: AsRef<[u8]>> {
    buffer: T,
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

impl<T: AsRef<[u8]>> Packet<T> {
    pub fn new(buffer: T) -> Result<Self, Error> {
        let len = buffer.as_ref().len();
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
        buf[field::DSCP_ECN] >> 2
    }

    #[inline]
    pub fn ecn(&self) -> u8 {
        let buf = self.buffer.as_ref();
        buf[field::DSCP_ECN] & 0b11
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

    pub fn checksum_valid(&self) -> bool {
        let buf = self.buffer.as_ref();
        checksum::compute(&buf[..self.header_len() as usize], 0) == 0
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Packet<&'a T> {
    #[inline]
    pub fn payload(&self) -> &'a [u8] {
        let buf = self.buffer.as_ref();
        &buf[self.header_len() as usize..]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Packet<T> {
    #[inline]
    pub fn set_version(&mut self, version: u8) {
        let mut buf = self.buffer.as_mut();
        buf[field::VER_IHL] = (buf[field::VER_IHL] & 0x0F) | ((version & 0x0F) << 4);
    }

    #[inline]
    pub fn set_header_len(&mut self, length: u8) {
        let mut buf = self.buffer.as_mut();
        buf[field::VER_IHL] = (buf[field::VER_IHL] & 0xF0) | ((length / 4) & 0x0F);
    }

    #[inline]
    pub fn set_dscp(&mut self, value: u8) {
        let mut buf = self.buffer.as_mut();
        buf[field::DSCP_ECN] = (buf[field::DSCP_ECN] & 0x03) | ((value << 2) & 0xFC);
    }

    #[inline]
    pub fn set_ecn(&mut self, value: u8) {
        let mut buf = self.buffer.as_mut();
        buf[field::DSCP_ECN] = (buf[field::DSCP_ECN] & 0xFC) | (value & 0x03);
    }

    #[inline]
    pub fn set_total_len(&mut self, length: u16) {
        let mut buf = self.buffer.as_mut();
        NetworkEndian::write_u16(&mut buf[field::LENGTH], length);
    }

    #[inline]
    pub fn set_identification(&mut self, value: u16) {
        let mut buf = self.buffer.as_mut();
        NetworkEndian::write_u16(&mut buf[field::ID], value)
    }

    /// Don't Fragment (DF) flag
    #[inline]
    pub fn set_flag_df(&mut self, flag: bool) {
        let mut buf = self.buffer.as_mut();
        buf[field::FLG_OFF.start] = (buf[field::FLG_OFF.start] & !0x40) |
                                    ((if flag { 1 << 7 } else { 0 }));
    }

    /// More Fragments (MF) flag
    #[inline]
    pub fn set_flag_mf(&mut self, flag: bool) {
        let mut buf = self.buffer.as_mut();
        buf[field::FLG_OFF.start] = (buf[field::FLG_OFF.start] & !0x20) |
                                    ((if flag { 1 << 6 } else { 0 }));
    }

    #[inline]
    pub fn set_ttl(&mut self, value: u8) {
        let mut buf = self.buffer.as_mut();
        buf[field::TTL] = value;
    }

    #[inline]
    pub fn set_protocol(&mut self, protocol: u8) {
        let mut buf = self.buffer.as_mut();
        buf[field::PROTOCOL] = protocol;
    }

    #[inline]
    pub fn set_checksum(&mut self, value: u16) {
        let mut buf = self.buffer.as_mut();
        NetworkEndian::write_u16(&mut buf[field::CHECKSUM], value);
    }

    #[inline]
    pub fn set_src_addr(&mut self, addr: &Address) {
        let mut buf = self.buffer.as_mut();
        NetworkEndian::write_u32(&mut buf[field::SRC_ADDR], addr.as_u32());
    }

    #[inline]
    pub fn set_dst_addr(&mut self, addr: &Address) {
        let mut buf = self.buffer.as_mut();
        NetworkEndian::write_u32(&mut buf[field::DST_ADDR], addr.as_u32());
    }
}

impl<'a, T: AsRef<[u8]> + AsMut<[u8]> + ?Sized> Packet<&'a mut T> {
    #[inline]
    pub fn payload_mut(&mut self) -> &mut [u8] {
        let start = self.header_len() as usize;
        let mut buf = self.buffer.as_mut();
        &mut buf[start..]
    }
}

impl<T: AsRef<[u8]>> fmt::Debug for Packet<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Ipv4Packet")
            .field("ihl", &self.header_len())
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
    pub fn parse<T: AsRef<[u8]> + ?Sized>(packet: &Packet<&T>) -> Result<Self, Error> {
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

        if !packet.checksum_valid() {
            return Err(Error::Checksum);
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

    pub fn send<T: AsRef<[u8]> + AsMut<[u8]>>(&self, packet: &mut Packet<T>) {
        packet.set_version(4);
        packet.set_header_len(field::DST_ADDR.end as u8);
        packet.set_dscp(0);
        packet.set_ecn(0);
        let len = packet.header_len() as u16 + self.payload_len as u16;
        packet.set_total_len(len);
        packet.set_identification(0);
        packet.set_flag_df(true);
        packet.set_flag_mf(false);
        packet.set_ttl(64);
        packet.set_protocol(TCP_PROTOCOL);
        packet.set_src_addr(&self.src_addr);
        packet.set_dst_addr(&self.dst_addr);
    }
}

pub mod checksum {
    use byteorder::{ByteOrder, NetworkEndian};
    use super::{TCP_PROTOCOL, Address};

    fn propogate_carries(word: u32) -> u16 {
        let mut word = word;
        while (word >> 16) != 0 {
            word = (word & 0xFFFF) + (word >> 16);
        }
        word as u16
    }

    pub fn compute(data: &[u8], start: u32) -> u16 {
        let mut sum = start;
        let mut i = 0;
        while i < data.len() {
            let word = if i + 2 <= data.len() {
                NetworkEndian::read_u16(&data[i..i + 2]).to_be() as u32
            } else {
                (data[i] as u32)
            };
            sum += word;
            i += 2;
        }
        sum = sum.to_be();

        !propogate_carries(sum)
    }

    pub fn pseudo_header(src_addr: &Address, dst_addr: &Address, length: u16) -> u32 {
        let src = src_addr.as_u32().to_be();
        let dst = dst_addr.as_u32().to_be();

        (src >> 16) + (src & 0xFFFF) + (dst >> 16) + (dst & 0xFFFF) + (length.to_be() as u32) +
        ((TCP_PROTOCOL as u16).to_be() as u32)
    }
}
