use crate::constants::{CURRENT_PROTOCOL_VERSION, HEADER_SIZE, MAGIC_BYTES, MAX_FRAME_SIZE};
use crate::error::ProtocolError;

/// The fixed 10-byte frame header preceding every serialized payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Magic identification bytes (`0x50, 0x4D`).
    pub magic: [u8; 2],
    /// Protocol version.
    pub version: u16,
    /// Bitflags for compression, encryption, etc.
    pub flags: u16,
    /// Byte length of the following serialized postcard payload.
    pub payload_len: u32,
}

impl FrameHeader {
    /// Constructs a standard v1 header for a payload of length `payload_len`.
    pub const fn new(payload_len: u32) -> Self {
        Self {
            magic: MAGIC_BYTES,
            version: CURRENT_PROTOCOL_VERSION,
            flags: 0,
            payload_len,
        }
    }

    /// Constructs a header with custom flags.
    pub const fn with_flags(flags: u16, payload_len: u32) -> Self {
        Self {
            magic: MAGIC_BYTES,
            version: CURRENT_PROTOCOL_VERSION,
            flags,
            payload_len,
        }
    }

    /// Encodes this header into a fixed 10-byte array.
    pub fn encode(&self, dst: &mut [u8; HEADER_SIZE]) {
        dst[0..2].copy_from_slice(&self.magic);
        dst[2..4].copy_from_slice(&self.version.to_le_bytes());
        dst[4..6].copy_from_slice(&self.flags.to_le_bytes());
        dst[6..10].copy_from_slice(&self.payload_len.to_le_bytes());
    }

    /// Decodes and validates a header from a fixed 10-byte slice.
    pub fn decode(src: &[u8; HEADER_SIZE]) -> Result<Self, ProtocolError> {
        let magic = [src[0], src[1]];
        if magic != MAGIC_BYTES {
            return Err(ProtocolError::InvalidMagic(magic[0], magic[1]));
        }

        let version = u16::from_le_bytes([src[2], src[3]]);
        if version != CURRENT_PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(version));
        }

        let flags = u16::from_le_bytes([src[4], src[5]]);
        let payload_len = u32::from_le_bytes([src[6], src[7], src[8], src[9]]);

        if payload_len as usize > MAX_FRAME_SIZE {
            return Err(ProtocolError::FrameTooLarge {
                size: payload_len as usize,
                max: MAX_FRAME_SIZE,
            });
        }

        Ok(Self {
            magic,
            version,
            flags,
            payload_len,
        })
    }
}
