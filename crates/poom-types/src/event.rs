use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::value::AttributeValue;

/// An instantaneous, point-in-time event or log record attached to a Span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    /// Timestamp in Unix nanoseconds when the event occurred.
    pub timestamp_unix_nanos: u64,
    /// Identifier or log message for the event.
    pub name: SmolStr,
    /// Contextual key-value attributes.
    pub attributes: BTreeMap<SmolStr, AttributeValue>,
}

impl EventRecord {
    /// Creates a new event with the current system time in nanoseconds.
    pub fn now(name: impl Into<SmolStr>) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        Self {
            timestamp_unix_nanos: nanos,
            name: name.into(),
            attributes: BTreeMap::new(),
        }
    }

    /// Creates an event with an explicit nanosecond timestamp.
    pub fn with_timestamp(timestamp_unix_nanos: u64, name: impl Into<SmolStr>) -> Self {
        Self {
            timestamp_unix_nanos,
            name: name.into(),
            attributes: BTreeMap::new(),
        }
    }

    /// Inserts an attribute into the event.
    pub fn with_attribute(
        mut self,
        key: impl Into<SmolStr>,
        val: impl Into<AttributeValue>,
    ) -> Self {
        self.attributes.insert(key.into(), val.into());
        self
    }
}
