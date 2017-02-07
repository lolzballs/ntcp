#[derive(Debug)]
pub enum Error {
    Malformed,
    Truncated,
    Unrecognized,
    Fragmented,
    UnknownProtocol,
    Checksum,
}

#[derive(Debug)]
pub enum SocketError {
    Closed,
}
