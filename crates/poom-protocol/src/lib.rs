pub mod codec;
pub mod constants;
pub mod error;
pub mod header;
pub mod message;

pub use codec::{FrameEncoder, StreamFrameDecoder};
pub use constants::{CURRENT_PROTOCOL_VERSION, HEADER_SIZE, MAGIC_BYTES, MAX_FRAME_SIZE};
pub use error::ProtocolError;
pub use header::FrameHeader;
pub use message::{ClientMessage, ServerMessage};
