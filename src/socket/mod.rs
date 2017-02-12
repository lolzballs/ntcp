mod interface;

use std::sync::mpsc;

use ::error::SocketError;
use ::tcp;

pub use self::interface::Interface as SocketInterface;

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

#[derive(Debug)]
pub struct Socket {
    pub endpoint: tcp::Endpoint,
    rx: mpsc::Receiver<PacketBuffer>,
    tx: mpsc::Sender<(tcp::Endpoint, PacketBuffer)>,
}

impl Socket {
    pub fn new(endpoint: tcp::Endpoint,
               rx: mpsc::Receiver<PacketBuffer>,
               tx: mpsc::Sender<(tcp::Endpoint, PacketBuffer)>)
               -> Self {
        Socket {
            endpoint: endpoint,
            rx: rx,
            tx: tx,
        }
    }

    pub fn recv(&mut self) -> Result<PacketBuffer, SocketError> {
        self.rx.recv().map_err(|_| SocketError::Closed)
    }

    pub fn send(&mut self, buf: PacketBuffer) -> Result<(), SocketError> {
        self.tx.send((self.endpoint, buf)).map_err(|_| SocketError::Closed)
    }
}
