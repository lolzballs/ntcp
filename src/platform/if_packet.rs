use libc;

pub const PACKET_RX_RING: libc::c_int = 5;

const TPACKET_ALIGNMENT: usize = 16;
macro_rules! tpacket_align {
    ($x: expr) => {
        (($x + TPACKET_ALIGNMENT - 1) & !(TPACKET_ALIGNMENT - 1))
    }
}

pub const TP_STATUS_USER: libc::c_ulong = 1 << 0;
pub const TP_STATUS_COPY: libc::c_ulong = 1 << 1;
pub const TP_STATUS_LOSING: libc::c_ulong = 1 << 2;

// TODO: Make these values use mem::size_of
pub const HEADER_SIZE: usize = 32;
pub const SOCKADDR_SIZE: usize = 20;
pub const TPACKET_HDR_LEN: usize = tpacket_align!(HEADER_SIZE) + tpacket_align!(SOCKADDR_SIZE) +
                                   tpacket_align!(6);

#[repr(C)]
#[derive(Debug)]
pub struct tpacket_hdr {
    pub tp_status: libc::c_ulong,
    pub tp_len: libc::c_uint,
    pub tp_snaplen: libc::c_uint,
    pub tp_mac: libc::c_ushort,
    pub tp_net: libc::c_ushort,
    pub tp_sec: libc::c_uint,
    pub tp_usec: libc::c_uint,
}

#[repr(C)]
pub struct tpacket_req {
    pub tp_block_size: libc::c_uint,
    pub tp_block_nr: libc::c_uint,
    pub tp_frame_size: libc::c_uint,
    pub tp_frame_nr: libc::c_uint,
}
