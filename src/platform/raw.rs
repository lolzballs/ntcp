use std::cmp;
use std::io;
use std::mem;
use std::ptr;
use std::slice;
use std::time::Instant;

use tcp;
use super::if_packet::*;

use libc;

const ETH_P_IP: u16 = 0x0800;

const RING_FRAME_CNT: usize = 7;
const RING_FRAME_LEN: usize = 262224;

pub struct MappedBuffer {
    base: *mut libc::c_void,
}

impl Default for MappedBuffer {
    fn default() -> Self {
        MappedBuffer { base: ptr::null_mut() }
    }
}

impl Drop for MappedBuffer {
    fn drop(&mut self) {
        if self.is_null() {
            return;
        }
        unsafe {
            (&mut *(self.base as *mut tpacket_hdr)).tp_status = 0;
        }
    }
}

impl MappedBuffer {
    pub fn is_null(&self) -> bool {
        self.base.is_null()
    }

    pub fn header(&self) -> &tpacket_hdr {
        unsafe { &*(self.base as *const tpacket_hdr) }
    }

    pub fn payload(&self) -> &[u8] {
        let len = self.header().tp_len;
        unsafe {
            slice::from_raw_parts(self.base.offset(TPACKET_HDR_LEN as isize) as *const u8,
                                  len as usize)
        }
    }
}

unsafe impl Send for RawSocket {}
unsafe impl Sync for RawSocket {}

pub struct RawSocket {
    recvfd: libc::c_int,
    sendfd: libc::c_int,

    block_size: usize,
    ring: *mut libc::c_void,
    ring_offset: usize,
}

impl RawSocket {
    pub fn new() -> io::Result<Self> {
        let recvfd = try!(Self::create_recv_socket());
        let sendfd = try!(Self::create_send_socket());

        let ring = try!(Self::init_ringbuffer(recvfd));

        Ok(RawSocket {
               recvfd: recvfd,
               sendfd: sendfd,
               block_size: 524288,
               ring: ring,
               ring_offset: 0,
           })
    }

    fn init_ringbuffer(fd: libc::c_int) -> io::Result<*mut libc::c_void> {
        let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u32 };

        let mut block_size = pagesize;
        while block_size < RING_FRAME_LEN as u32 {
            block_size <<= 1;
        }

        let tp = tpacket_req {
            tp_block_size: block_size,
            tp_block_nr: 7,
            tp_frame_size: 262224,
            tp_frame_nr: 7,
        };

        let res = unsafe {
            libc::setsockopt(fd,
                             libc::SOL_PACKET,
                             PACKET_RX_RING,
                             &tp as *const _ as *const libc::c_void,
                             mem::size_of::<tpacket_req>() as u32)
        };
        if res < 0 {
            return Err(io::Error::last_os_error());
        }

        let res = unsafe {
            libc::mmap(ptr::null_mut(),
                       (tp.tp_block_size * tp.tp_block_nr) as usize,
                       libc::PROT_READ | libc::PROT_WRITE,
                       libc::MAP_SHARED,
                       fd,
                       0)
        };
        if res.is_null() {
            return Err(io::Error::new(io::ErrorKind::Other, "mmap failed"));
        }

        Ok(res)
    }

    fn create_recv_socket() -> io::Result<libc::c_int> {
        let sockfd =
            unsafe { libc::socket(libc::AF_PACKET, libc::SOCK_DGRAM, ETH_P_IP.to_be() as i32) };
        if sockfd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(sockfd)
    }

    fn create_send_socket() -> io::Result<libc::c_int> {
        let sockfd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_RAW, libc::IPPROTO_TCP) };
        if sockfd < 0 {
            return Err(io::Error::last_os_error());
        }

        let on: u8 = 1;
        let res = unsafe {
            libc::setsockopt(sockfd,
                             libc::IPPROTO_IP,
                             libc::IP_HDRINCL,
                             &on as *const _ as *const libc::c_void,
                             1)
        };

        if res < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(sockfd)
    }

    pub fn recv(&self, index: usize) -> io::Result<MappedBuffer> {
        let index = index % RING_FRAME_CNT;
        let mut header = unsafe {
            // TODO: Some checking
            let pointer = self.ring.offset((index * self.block_size) as isize) as *mut tpacket_hdr;
            &mut *pointer
        };

        while header.tp_status & TP_STATUS_USER == 0 {
            let mut pollset = libc::pollfd {
                fd: self.recvfd,
                events: libc::POLLIN,
                revents: 0,
            };
            let ret = unsafe { libc::poll(&mut pollset as *mut libc::pollfd, 1, -1) };
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(MappedBuffer { base: header as *mut tpacket_hdr as *mut libc::c_void })
    }

    pub fn send(&self, dest: tcp::Endpoint, buffer: &[u8]) -> io::Result<usize> {
        let addr = libc::sockaddr_in {
            sin_family: libc::AF_INET as u16,
            sin_port: 0,
            sin_addr: libc::in_addr { s_addr: dest.addr.as_u32().to_be() },
            sin_zero: [0; 8],
        };

        unsafe {
            let res = libc::sendto(self.sendfd,
                                   buffer.as_ptr() as *const libc::c_void,
                                   buffer.len(),
                                   0,
                                   &addr as *const libc::sockaddr_in as *const libc::sockaddr,
                                   mem::size_of::<libc::sockaddr_in>() as u32);

            if res < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(res as usize)
        }
    }
}

impl Drop for RawSocket {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.recvfd);
            libc::close(self.sendfd);
        }
    }
}
