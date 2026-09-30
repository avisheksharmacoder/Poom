use bytes::{Buf, BufMut, BytesMut};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::constants::{HEADER_SIZE, MAX_FRAME_SIZE};
use crate::error::ProtocolError;
use crate::header::FrameHeader;
use crate::message::{ClientMessage, ServerMessage};

/// Encodes structured messages into length-prefixed postcard byte frames.
pub struct FrameEncoder;

impl FrameEncoder {
    /// Encodes a `ClientMessage` into the destination `BytesMut` buffer.
    pub fn encode_client_message(
        msg: &ClientMessage,
        dst: &mut BytesMut,
    ) -> Result<(), ProtocolError> {
        Self::encode_payload(msg, dst)
    }

    /// Encodes a `ServerMessage` into the destination `BytesMut` buffer.
    pub fn encode_server_message(
        msg: &ServerMessage,
        dst: &mut BytesMut,
    ) -> Result<(), ProtocolError> {
        Self::encode_payload(msg, dst)
    }

    /// Generic helper encoding any serializable payload with a 10-byte length-prefixed header.
    pub fn encode_payload<T: Serialize>(
        val: &T,
        dst: &mut BytesMut,
    ) -> Result<(), ProtocolError> {
        let payload = postcard::to_stdvec(val)
            .map_err(|e| ProtocolError::SerializationFailed(e.to_string()))?;

        let payload_len = payload.len();
        if payload_len > MAX_FRAME_SIZE {
            return Err(ProtocolError::FrameTooLarge {
                size: payload_len,
                max: MAX_FRAME_SIZE,
            });
        }

        dst.reserve(HEADER_SIZE + payload_len);

        let header = FrameHeader::new(payload_len as u32);
        let mut header_buf = [0u8; HEADER_SIZE];
        header.encode(&mut header_buf);

        dst.put_slice(&header_buf);
        dst.put_slice(&payload);

        Ok(())
    }
}

/// Decodes sliding-window byte buffers into strongly typed messages.
pub struct StreamFrameDecoder;

impl StreamFrameDecoder {
    /// Decodes a `ClientMessage` from an incoming stream buffer.
    ///
    /// Returns:
    /// - `Ok(Some(msg))` when a full, valid frame has been decoded and removed from `src`.
    /// - `Ok(None)` when more bytes are needed from the socket.
    /// - `Err(e)` on corrupted magic bytes, oversized frames, or deserialization failures.
    pub fn decode_client_message(
        src: &mut BytesMut,
    ) -> Result<Option<ClientMessage>, ProtocolError> {
        Self::decode_payload(src)
    }

    /// Decodes a `ServerMessage` from an incoming stream buffer.
    pub fn decode_server_message(
        src: &mut BytesMut,
    ) -> Result<Option<ServerMessage>, ProtocolError> {
        Self::decode_payload(src)
    }

    /// Generic sliding-window frame extractor.
    pub fn decode_payload<T: DeserializeOwned>(
        src: &mut BytesMut,
    ) -> Result<Option<T>, ProtocolError> {
        // Step 1: Wait for at least the 10-byte header
        if src.len() < HEADER_SIZE {
            return Ok(None);
        }

        // Step 2: Decode and validate header without consuming bytes yet
        let mut header_buf = [0u8; HEADER_SIZE];
        header_buf.copy_from_slice(&src[..HEADER_SIZE]);
        let header = FrameHeader::decode(&header_buf)?;

        let payload_len = header.payload_len as usize;
        let total_frame_len = HEADER_SIZE + payload_len;

        // Step 3: Check if the complete payload has arrived
        if src.len() < total_frame_len {
            // Pre-reserve capacity for the remainder of the incoming frame
            src.reserve(total_frame_len - src.len());
            return Ok(None);
        }

        // Step 4: Advance past the header
        src.advance(HEADER_SIZE);

        // Step 5: Extract payload slice zero-copy
        let payload_bytes = src.split_to(payload_len);

        // Step 6: Postcard deserialization
        let item: T = postcard::from_bytes(&payload_bytes)
            .map_err(|e| ProtocolError::DeserializationFailed(e.to_string()))?;

        Ok(Some(item))
    }
}
