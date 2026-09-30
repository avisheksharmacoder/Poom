use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::TypeError;

/// 128-bit globally unique identifier for a Trace, backed by UUIDv7.
///
/// Guaranteed to be time-ordered down to millisecond precision, enabling
/// sequential B-Tree appends and range scans in `redb` without secondary indexing.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[repr(transparent)]
pub struct TraceId(Uuid);

/// 128-bit globally unique identifier for a Span, backed by UUIDv7.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[repr(transparent)]
pub struct SpanId(Uuid);

impl TraceId {
    /// Generates a new time-ordered UUIDv7 TraceId.
    #[inline]
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Creates a TraceId from a raw 16-byte array.
    #[inline]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    /// Returns the raw 16-byte big-endian representation.
    #[inline]
    pub const fn to_bytes(&self) -> [u8; 16] {
        *self.0.as_bytes()
    }

    /// Borrows the raw 16-byte slice.
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    /// Extracts the Unix millisecond timestamp encoded in the UUIDv7 bits.
    pub fn timestamp_millis(&self) -> u64 {
        if let Some(ts) = self.0.get_timestamp() {
            let (secs, nanos) = ts.to_unix();
            secs * 1000 + (nanos as u64 / 1_000_000)
        } else {
            0
        }
    }

    /// Returns the underlying `uuid::Uuid`.
    #[inline]
    pub const fn into_inner(self) -> Uuid {
        self.0
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::generate()
    }
}

impl fmt::Debug for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TraceId({})", self.0)
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for TraceId {
    type Err = TypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uuid = Uuid::parse_str(s).map_err(|e| TypeError::InvalidId(e.to_string()))?;
        Ok(Self(uuid))
    }
}

impl From<Uuid> for TraceId {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<TraceId> for Uuid {
    #[inline]
    fn from(id: TraceId) -> Self {
        id.0
    }
}

// ---------------------------------------------------------------------------
// SpanId Implementation
// ---------------------------------------------------------------------------

impl SpanId {
    /// Generates a new time-ordered UUIDv7 SpanId.
    #[inline]
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Creates a SpanId from a raw 16-byte array.
    #[inline]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    /// Returns the raw 16-byte big-endian representation.
    #[inline]
    pub const fn to_bytes(&self) -> [u8; 16] {
        *self.0.as_bytes()
    }

    /// Borrows the raw 16-byte slice.
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    /// Extracts the Unix millisecond timestamp encoded in the UUIDv7 bits.
    pub fn timestamp_millis(&self) -> u64 {
        if let Some(ts) = self.0.get_timestamp() {
            let (secs, nanos) = ts.to_unix();
            secs * 1000 + (nanos as u64 / 1_000_000)
        } else {
            0
        }
    }

    /// Returns the underlying `uuid::Uuid`.
    #[inline]
    pub const fn into_inner(self) -> Uuid {
        self.0
    }
}

impl Default for SpanId {
    fn default() -> Self {
        Self::generate()
    }
}

impl fmt::Debug for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SpanId({})", self.0)
    }
}

impl fmt::Display for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SpanId {
    type Err = TypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uuid = Uuid::parse_str(s).map_err(|e| TypeError::InvalidId(e.to_string()))?;
        Ok(Self(uuid))
    }
}

impl From<Uuid> for SpanId {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<SpanId> for Uuid {
    #[inline]
    fn from(id: SpanId) -> Self {
        id.0
    }
}
