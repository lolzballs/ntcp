use super::error::Error;
use super::ipv4;

use std::fmt;

use byteorder::{ByteOrder, NetworkEndian};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
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

pub struct Packet<T: AsRef<[u8]>> {
    pub buffer: T,
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

impl<T: AsRef<[u8]>> Packet<T> {
    pub fn new(buffer: T) -> Result<Self, Error> {
        let len = buffer.as_ref().len();
        if len < field::URGENT.end {
            Err(Error::Truncated)
        } else {
            Ok(Packet { buffer: buffer })
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
        buf[field::OFF_FLG.end - 1] & 0x80 != 0
    }

    #[inline]
    pub fn flag_ece(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end - 1] & 0x40 != 0
    }

    #[inline]
    pub fn flag_urg(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end - 1] & 0x20 != 0
    }

    #[inline]
    pub fn flag_ack(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end - 1] & 0x10 != 0
    }

    #[inline]
    pub fn flag_psh(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end - 1] & 0x08 != 0
    }

    #[inline]
    pub fn flag_rst(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end - 1] & 0x04 != 0
    }

    #[inline]
    pub fn flag_syn(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end - 1] & 0x02 != 0
    }

    #[inline]
    pub fn flag_fin(&self) -> bool {
        let buf = self.buffer.as_ref();
        buf[field::OFF_FLG.end - 1] & 0x01 != 0
    }

    #[inline]
    pub fn window_size(&self) -> u16 {
        let buf = self.buffer.as_ref();
        NetworkEndian::read_u16(&buf[field::WINDOW_SIZE])
    }

    #[inline]
    pub fn checksum(&self) -> u16 {
        let buf = self.buffer.as_ref();
        NetworkEndian::read_u16(&buf[field::CHECKSUM])
    }

    #[inline]
    pub fn urgent(&self) -> u16 {
        let buf = self.buffer.as_ref();
        NetworkEndian::read_u16(&buf[field::URGENT])
    }

    pub fn checksum_valid(&self, src_addr: &ipv4::Address, dst_addr: &ipv4::Address) -> bool {
        use ipv4::checksum;
        let buf = self.buffer.as_ref();
        checksum::compute(&buf,
                          checksum::pseudo_header(src_addr, dst_addr, buf.len() as u16)) ==
        0
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Packet<&'a T> {
    #[inline]
    pub fn payload(&self) -> &'a [u8] {
        let len = (self.data_offset()) as usize;
        let buf = self.buffer.as_ref();
        &buf[len..]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Packet<T> {
    #[inline]
    pub fn set_src_port(&mut self, port: u16) {
        let mut buf = self.buffer.as_mut();
        NetworkEndian::write_u16(&mut buf[field::SRC_PORT], port);
    }

    #[inline]
    pub fn set_dst_port(&mut self, port: u16) {
        let mut buf = self.buffer.as_mut();
        NetworkEndian::write_u16(&mut buf[field::DST_PORT], port);
    }

    #[inline]
    pub fn set_seq_num(&mut self, value: u32) {
        let mut buf = self.buffer.as_mut();
        NetworkEndian::write_u32(&mut buf[field::SEQ_NUM], value);
    }

    #[inline]
    pub fn set_ack_num(&mut self, value: u32) {
        let mut buf = self.buffer.as_mut();
        NetworkEndian::write_u32(&mut buf[field::ACK_NUM], value);
    }

    #[inline]
    pub fn set_data_offset(&mut self, offset: u8) {
        let mut buf = self.buffer.as_mut();
        buf[field::OFF_FLG.start] = (buf[field::OFF_FLG.start] & 0x0F) | ((offset / 4) << 4);
    }

    #[inline]
    pub fn clear_flags(&mut self) {
        let offset = self.data_offset();
        let mut buf = self.buffer.as_mut();
        buf[field::OFF_FLG.start] = (offset / 4) << 4;
        buf[field::OFF_FLG.end - 1] = 0;
    }

    #[inline]
    pub fn set_flag_ns(&mut self, flag: bool) {
        let mut buf = self.buffer.as_mut();
        if flag {
            buf[field::OFF_FLG.start] |= 0x01;
        } else {
            buf[field::OFF_FLG.start] &= !0x01;
        }
    }

    #[inline]
    pub fn set_flag_cwr(&mut self, flag: bool) {
        let mut buf = self.buffer.as_mut();
        if flag {
            buf[field::OFF_FLG.end - 1] |= 0x80;
        } else {
            buf[field::OFF_FLG.end - 1] &= !0x80;
        }
    }

    #[inline]
    pub fn set_flag_ece(&mut self, flag: bool) {
        let mut buf = self.buffer.as_mut();
        if flag {
            buf[field::OFF_FLG.end - 1] |= 0x40;
        } else {
            buf[field::OFF_FLG.end - 1] &= !0x40;
        }
    }

    #[inline]
    pub fn set_flag_urg(&mut self, flag: bool) {
        let mut buf = self.buffer.as_mut();
        if flag {
            buf[field::OFF_FLG.end - 1] |= 0x20;
        } else {
            buf[field::OFF_FLG.end - 1] &= !0x20;
        }
    }

    #[inline]
    pub fn set_flag_ack(&mut self, flag: bool) {
        let mut buf = self.buffer.as_mut();
        if flag {
            buf[field::OFF_FLG.end - 1] |= 0x10;
        } else {
            buf[field::OFF_FLG.end - 1] &= !0x10;
        }
    }

    #[inline]
    pub fn set_flag_psh(&mut self, flag: bool) {
        let mut buf = self.buffer.as_mut();
        if flag {
            buf[field::OFF_FLG.end - 1] |= 0x08;
        } else {
            buf[field::OFF_FLG.end - 1] &= !0x08;
        }
    }

    #[inline]
    pub fn set_flag_rst(&mut self, flag: bool) {
        let mut buf = self.buffer.as_mut();
        if flag {
            buf[field::OFF_FLG.end - 1] |= 0x04;
        } else {
            buf[field::OFF_FLG.end - 1] &= !0x04;
        }
    }

    #[inline]
    pub fn set_flag_syn(&mut self, flag: bool) {
        let mut buf = self.buffer.as_mut();
        if flag {
            buf[field::OFF_FLG.end - 1] |= 0x02;
        } else {
            buf[field::OFF_FLG.end - 1] &= !0x02;
        }
    }

    #[inline]
    pub fn set_flag_fin(&mut self, flag: bool) {
        let mut buf = self.buffer.as_mut();
        if flag {
            buf[field::OFF_FLG.end - 1] |= 0x01;
        } else {
            buf[field::OFF_FLG.end - 1] &= !0x01;
        }
    }

    #[inline]
    pub fn set_window_size(&mut self, value: u16) {
        let mut buf = self.buffer.as_mut();
        NetworkEndian::write_u16(&mut buf[field::WINDOW_SIZE], value);
    }

    #[inline]
    pub fn set_checksum(&mut self, value: u16) {
        let mut buf = self.buffer.as_mut();
        NetworkEndian::write_u16(&mut buf[field::CHECKSUM], value);
    }

    #[inline]
    pub fn set_urgent(&mut self, value: u16) {
        let mut buf = self.buffer.as_mut();
        NetworkEndian::write_u16(&mut buf[field::URGENT], value);
    }

    #[inline]
    pub fn fill_checksum(&mut self, src_addr: &ipv4::Address, dst_addr: &ipv4::Address) {
        use ipv4::checksum;
        self.set_checksum(0);

        let sum = {
            let buf = self.buffer.as_ref();
            checksum::compute(&buf,
                              checksum::pseudo_header(src_addr, dst_addr, buf.len() as u16))
        };

        self.set_checksum(sum);
    }
}

impl<'a, T: AsRef<[u8]> + AsMut<[u8]> + ?Sized> Packet<&'a mut T> {
    #[inline]
    pub fn payload(&mut self) -> &mut [u8] {
        let len = (self.data_offset()) as usize;
        let mut buf = self.buffer.as_mut();
        &mut buf[len..]
    }
}

impl<T: AsRef<[u8]>> fmt::Debug for Packet<T> {
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

#[derive(Debug, PartialEq)]
pub enum Control {
    None,
    Syn,
    Fin,
    Rst,
}

#[derive(Debug)]
pub struct Repr<'a> {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq: u32,
    pub ack: Option<u32>,
    pub control: Control,
    pub payload: &'a [u8],
}

impl<'a> Repr<'a> {
    pub fn parse<T: ?Sized>(packet: &Packet<&'a T>,
                            src_addr: &ipv4::Address,
                            dst_addr: &ipv4::Address)
                            -> Result<Self, Error>
        where T: AsRef<[u8]>
    {
        if packet.src_port() == 0 {
            return Err(Error::Malformed);
        }
        if packet.dst_port() == 0 {
            return Err(Error::Malformed);
        }

        // if !packet.checksum_valid(src_addr, dst_addr) {
        //    return Err(Error::Checksum);
        // }

        let control = if packet.flag_syn() {
            Control::Syn
        } else if packet.flag_fin() {
            Control::Fin
        } else if packet.flag_rst() {
            Control::Rst
        } else {
            Control::None
        };

        let ack_num = if packet.flag_ack() {
            Some(packet.ack_num())
        } else {
            None
        };

        Ok(Repr {
            src_port: packet.src_port(),
            dst_port: packet.dst_port(),
            seq: packet.seq_num(),
            ack: ack_num,
            control: control,
            payload: packet.payload(),
        })
    }

    pub fn header_len(&self) -> usize {
        field::URGENT.end
    }

    pub fn emit<T: ?Sized>(&self,
                           packet: &mut Packet<&mut T>,
                           src_addr: &ipv4::Address,
                           dst_addr: &ipv4::Address)
        where T: AsRef<[u8]> + AsMut<[u8]>
    {
        packet.set_src_port(self.src_port);
        packet.set_dst_port(self.dst_port);
        packet.set_seq_num(self.seq);
        packet.set_ack_num(self.ack.unwrap_or(0));
        packet.set_data_offset(self.header_len() as u8);
        packet.clear_flags();
        match self.control {
            Control::None => (),
            Control::Syn => packet.set_flag_syn(true),
            Control::Fin => packet.set_flag_fin(true),
            Control::Rst => packet.set_flag_rst(true),
        };
        if self.ack.is_some() {
            packet.set_flag_ack(true);
        }
        packet.payload().copy_from_slice(self.payload);
        packet.fill_checksum(src_addr, dst_addr);
    }
}
