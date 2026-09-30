use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// Dynamic value representation for span attributes and metadata.
///
/// Uses `BTreeMap` instead of `HashMap` to ensure canonical, deterministic
/// binary serialization order across all runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttributeValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Smol(SmolStr),
    Bytes(Vec<u8>),
    Array(Vec<AttributeValue>),
    Map(BTreeMap<SmolStr, AttributeValue>),
}

impl AttributeValue {
    /// Attempts to borrow as a string slice.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            Self::Smol(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Attempts to read as a 64-bit signed integer.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Attempts to read as a 64-bit float.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Attempts to read as a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Returns `true` if the value is `Null`.
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

impl PartialEq for AttributeValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => {
                if a.is_nan() && b.is_nan() {
                    true
                } else {
                    a == b
                }
            }
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Smol(a), Self::Smol(b)) => a == b,
            (Self::String(a), Self::Smol(b)) => a.as_str() == b.as_str(),
            (Self::Smol(a), Self::String(b)) => a.as_str() == b.as_str(),
            (Self::Bytes(a), Self::Bytes(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Map(a), Self::Map(b)) => a == b,
            _ => false,
        }
    }
}

impl Default for AttributeValue {
    #[inline]
    fn default() -> Self {
        Self::Null
    }
}

// ---------------------------------------------------------------------------
// Ergonomic From Conversions
// ---------------------------------------------------------------------------

impl From<bool> for AttributeValue {
    #[inline]
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i32> for AttributeValue {
    #[inline]
    fn from(v: i32) -> Self {
        Self::Int(v as i64)
    }
}

impl From<i64> for AttributeValue {
    #[inline]
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<u32> for AttributeValue {
    #[inline]
    fn from(v: u32) -> Self {
        Self::Int(v as i64)
    }
}

impl From<u64> for AttributeValue {
    #[inline]
    fn from(v: u64) -> Self {
        Self::Int(v as i64)
    }
}

impl From<f32> for AttributeValue {
    #[inline]
    fn from(v: f32) -> Self {
        Self::Float(v as f64)
    }
}

impl From<f64> for AttributeValue {
    #[inline]
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<&str> for AttributeValue {
    #[inline]
    fn from(v: &str) -> Self {
        if v.len() <= 23 {
            Self::Smol(SmolStr::new(v))
        } else {
            Self::String(v.to_string())
        }
    }
}

impl From<String> for AttributeValue {
    #[inline]
    fn from(v: String) -> Self {
        if v.len() <= 23 {
            Self::Smol(SmolStr::new(&v))
        } else {
            Self::String(v)
        }
    }
}

impl From<SmolStr> for AttributeValue {
    #[inline]
    fn from(v: SmolStr) -> Self {
        Self::Smol(v)
    }
}

impl From<Vec<u8>> for AttributeValue {
    #[inline]
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes(v)
    }
}

impl From<serde_json::Value> for AttributeValue {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Self::Int(i)
                } else if let Some(f) = n.as_f64() {
                    Self::Float(f)
                } else {
                    Self::String(n.to_string())
                }
            }
            serde_json::Value::String(s) => s.into(),
            serde_json::Value::Array(arr) => {
                Self::Array(arr.into_iter().map(AttributeValue::from).collect())
            }
            serde_json::Value::Object(obj) => {
                let mut map = BTreeMap::new();
                for (k, v) in obj {
                    map.insert(SmolStr::new(k), AttributeValue::from(v));
                }
                Self::Map(map)
            }
        }
    }
}
