use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("Invalid magic bytes: expected [0x50, 0x4D], got [{0:#04x}, {1:#04x}]")]
    InvalidMagic(u8, u8),

    #[error("Unsupported protocol version: {0}")]
    UnsupportedVersion(u16),

    #[error("Frame size {size} exceeds maximum allowable limit of {max} bytes")]
    FrameTooLarge { size: usize, max: usize },

    #[error("Failed to serialize payload via postcard: {0}")]
    SerializationFailed(String),

    #[error("Failed to deserialize payload via postcard: {0}")]
    DeserializationFailed(String),

    #[error("Unexpected EOF: required {expected} bytes, available {available}")]
    UnexpectedEof { expected: usize, available: usize },
}
