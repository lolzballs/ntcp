mod interface;
mod socket;

use ::error::SocketError;

pub use self::interface::Interface as SocketInterface;
pub use self::socket::Socket;

#[derive(Debug)]
pub struct PacketBuffer {
    pub payload: Box<[u8]>,
}

impl PacketBuffer {
    pub fn new(payload: &[u8]) -> Self {
        PacketBuffer { payload: payload.to_vec().into_boxed_slice() }
    }
}

#[derive(Debug, PartialEq)]
enum SocketState {
    SynSent,
    SynReceived,
    Established,
    Closed,
}
