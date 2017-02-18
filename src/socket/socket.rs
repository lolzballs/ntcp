use std::cmp;
use std::io;
use std::sync::mpsc;

use super::{PacketBuffer, SocketError};
use ::tcp;

#[derive(Debug)]
pub struct Socket {
    pub endpoint: tcp::Endpoint,
    rx: mpsc::Receiver<PacketBuffer>,
    tx: mpsc::Sender<(tcp::Endpoint, PacketBuffer)>,

    rx_buffer: Vec<u8>,
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
            rx_buffer: Vec::new(),
        }
    }

    fn recv(&mut self) -> Result<PacketBuffer, SocketError> {
        self.rx.recv().map_err(|_| SocketError::Closed)
    }

    fn send(&mut self, buf: PacketBuffer) -> Result<(), SocketError> {
        self.tx.send((self.endpoint, buf)).map_err(|_| SocketError::Closed)
    }

    pub fn to_tx_rx
        (self)
         -> (mpsc::Sender<(tcp::Endpoint, PacketBuffer)>, mpsc::Receiver<PacketBuffer>) {
        (self.tx, self.rx)
    }
}

impl io::Write for Socket {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.send(PacketBuffer::new(buf)).map(|_| buf.len()).map_err(|err| {
            match err {
                SocketError::Closed => io::Error::from(io::ErrorKind::NotConnected),
                _ => io::Error::new(io::ErrorKind::Other, "Something else"),
            }
        })
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}

impl io::Read for Socket {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        let mut read_len = 0;

        // Read from buffer if there is any
        if !self.rx_buffer.is_empty() {
            let len = self.rx_buffer.len();
            let end = cmp::min(buf.len(), len);
            read_len += end;

            let bytes: Vec<_> = self.rx_buffer.drain(..end).collect();
            buf.copy_from_slice(bytes.as_slice());
        }

        self.recv()
            .map(|recv| {
                let mut buf = &mut buf[read_len..];
                let len = cmp::min(recv.payload.len(), buf.len());
                let mut buf = &mut buf[..len];
                buf.copy_from_slice(&recv.payload[..len]);

                if len > buf.len() {
                    self.rx_buffer.extend_from_slice(&recv.payload[buf.len()..]);
                }

                read_len + buf.len()
            })
            .map_err(|err| {
                match err {
                    SocketError::Closed => io::Error::from(io::ErrorKind::NotConnected),
                    _ => io::Error::new(io::ErrorKind::Other, "Something else"),
                }
            })
    }
}
