use libc;
use std::io;
use std::mem;
use super::tcp;

const ETH_P_IP: u16 = 0x0800;

pub struct RawSocket {
    recvfd: libc::c_int,
    sendfd: libc::c_int,
}

impl RawSocket {
    pub fn new() -> io::Result<Self> {
        let recvfd = try!(Self::create_recv_socket());
        let sendfd = try!(Self::create_send_socket());
        Ok(RawSocket {
            recvfd: recvfd,
            sendfd: sendfd,
        })
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

    pub fn recv(&self, buffer: &mut [u8]) -> io::Result<usize> {
        unsafe {
            let res = libc::recv(self.recvfd,
                                 buffer.as_mut_ptr() as *mut libc::c_void,
                                 buffer.len(),
                                 0);
            if res < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(res as usize)
        }
    }

    pub fn send(&self, dest: tcp::Endpoint, buffer: &[u8]) -> io::Result<usize> {
        let addr = libc::sockaddr_in {
            sin_family: libc::AF_INET as u16,
            sin_port: dest.port.to_be(),
            sin_addr: libc::in_addr { s_addr: dest.addr.as_beu32() },
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
