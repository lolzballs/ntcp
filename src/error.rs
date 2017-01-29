#[derive(Debug)]
pub enum Error {
    Malformed,
    Truncated,
    Unrecognized,
    Fragmented,
    UnknownProtocol,
}
