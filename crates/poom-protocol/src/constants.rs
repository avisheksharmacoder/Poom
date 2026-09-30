/// Magic identification bytes at the start of every Poom frame: ASCII "PM" (0x50, 0x4D).
pub const MAGIC_BYTES: [u8; 2] = [0x50, 0x4D];

/// Active binary protocol version.
pub const CURRENT_PROTOCOL_VERSION: u16 = 1;

/// Fixed wire frame header length:
/// - 2 bytes: Magic
/// - 2 bytes: Protocol version (Little-Endian)
/// - 2 bytes: Flags (Little-Endian)
/// - 4 bytes: Payload length (Little-Endian)
pub const HEADER_SIZE: usize = 10;

/// Maximum permissible frame size in bytes (64 MB).
/// Any frame length exceeding this is rejected before memory is allocated.
pub const MAX_FRAME_SIZE: usize = 64 * 1024 * 1024;
