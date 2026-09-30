use poom_types::TraceId;

/// Encodes a timestamp and TraceId into a 24-byte big-endian composite key.
///
/// Layout:
/// - `[0..8]`: Timestamp in nanoseconds encoded as Big-Endian u64.
/// - `[8..24]`: 16-byte raw TraceId.
#[inline]
pub fn encode_time_index_key(timestamp_nanos: u64, trace_id: TraceId) -> [u8; 24] {
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&timestamp_nanos.to_be_bytes());
    key[8..24].copy_from_slice(trace_id.as_bytes());
    key
}

/// Unpacks a 24-byte composite key back into its nanosecond timestamp and TraceId.
#[inline]
pub fn decode_time_index_key(key: &[u8; 24]) -> (u64, TraceId) {
    let mut ts_bytes = [0u8; 8];
    ts_bytes.copy_from_slice(&key[0..8]);
    let timestamp_nanos = u64::from_be_bytes(ts_bytes);

    let mut trace_bytes = [0u8; 16];
    trace_bytes.copy_from_slice(&key[8..24]);
    let trace_id = TraceId::from_bytes(trace_bytes);

    (timestamp_nanos, trace_id)
}

/// Encodes a tag key, tag value, and TraceId into a composite byte slice for prefix indexing.
///
/// Layout:
/// - `[0..2]`: Key length K (u16 Little-Endian).
/// - `[2..2+K]`: Key bytes.
/// - `[2+K..4+K]`: Value length V (u16 Little-Endian).
/// - `[4+K..4+K+V]`: Value bytes.
/// - `[4+K+V..20+K+V]`: 16-byte TraceId.
pub fn encode_tag_index_key(key: &str, val: &str, trace_id: TraceId) -> Vec<u8> {
    let key_bytes = key.as_bytes();
    let val_bytes = val.as_bytes();
    let mut buf = Vec::with_capacity(2 + key_bytes.len() + 2 + val_bytes.len() + 16);

    buf.extend_from_slice(&(key_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(key_bytes);
    buf.extend_from_slice(&(val_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(val_bytes);
    buf.extend_from_slice(trace_id.as_bytes());

    buf
}

/// Encodes a tag key and value prefix for range scanning.
pub fn encode_tag_prefix(key: &str, val: &str) -> Vec<u8> {
    let key_bytes = key.as_bytes();
    let val_bytes = val.as_bytes();
    let mut buf = Vec::with_capacity(2 + key_bytes.len() + 2 + val_bytes.len());

    buf.extend_from_slice(&(key_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(key_bytes);
    buf.extend_from_slice(&(val_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(val_bytes);

    buf
}

/// Extracts the trailing 16-byte TraceId from a composite tag index key.
pub fn extract_trace_id_from_tag_key(key: &[u8]) -> Option<TraceId> {
    if key.len() < 16 {
        return None;
    }
    let offset = key.len() - 16;
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&key[offset..]);
    Some(TraceId::from_bytes(bytes))
}
