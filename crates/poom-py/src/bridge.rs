use std::collections::BTreeMap;
use std::str::FromStr;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString};
use smol_str::SmolStr;

use poom_types::{AttributeValue, SpanKind, SpanStatus};

/// Converts a dynamic Python object into a strongly typed `AttributeValue`.
pub fn py_to_attribute_value(obj: &Bound<'_, PyAny>) -> AttributeValue {
    if obj.is_none() {
        return AttributeValue::Null;
    }

    if let Ok(b) = obj.downcast::<PyBool>() {
        return AttributeValue::Bool(b.is_true());
    }

    if let Ok(i) = obj.downcast::<PyInt>() {
        if let Ok(val) = i.extract::<i64>() {
            return AttributeValue::Int(val);
        }
    }

    if let Ok(f) = obj.downcast::<PyFloat>() {
        if let Ok(val) = f.extract::<f64>() {
            return AttributeValue::Float(val);
        }
    }

    if let Ok(s) = obj.downcast::<PyString>() {
        if let Ok(val) = s.extract::<std::borrow::Cow<'_, str>>() {
            return AttributeValue::from(val.as_ref());
        }
    }

    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut map = BTreeMap::new();
        for (k, v) in dict {
            if let Ok(k_str) = k.extract::<String>() {
                map.insert(SmolStr::new(k_str), py_to_attribute_value(&v));
            }
        }
        return AttributeValue::Map(map);
    }

    if let Ok(list) = obj.downcast::<PyList>() {
        let mut arr = Vec::with_capacity(list.len());
        for item in list {
            arr.push(py_to_attribute_value(&item));
        }
        return AttributeValue::Array(arr);
    }

    // Fallback: convert to string representation
    if let Ok(repr) = obj.str() {
        if let Ok(s) = repr.extract::<std::borrow::Cow<'_, str>>() {
            return AttributeValue::from(s.as_ref());
        }
    }

    AttributeValue::Null
}

/// Parses a span kind string into `SpanKind`.
pub fn parse_span_kind(kind_str: Option<&str>) -> SpanKind {
    kind_str
        .and_then(|s| SpanKind::from_str(s).ok())
        .unwrap_or(SpanKind::Function)
}

/// Constructs a `SpanStatus` from Python error attributes.
pub fn build_span_status(
    is_error: bool,
    error_type: Option<&str>,
    error_message: Option<&str>,
    backtrace: Option<&str>,
) -> SpanStatus {
    if !is_error {
        SpanStatus::Ok
    } else {
        let err_type = error_type.unwrap_or("Exception");
        let msg = error_message.unwrap_or("Unknown error");
        if let Some(bt) = backtrace {
            SpanStatus::error_with_backtrace(err_type, msg, bt)
        } else {
            SpanStatus::error(err_type, msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_span_kind() {
        assert_eq!(parse_span_kind(Some("llm")), SpanKind::Llm);
        assert_eq!(parse_span_kind(Some("tool")), SpanKind::Tool);
        assert_eq!(parse_span_kind(Some("chain")), SpanKind::Chain);
        assert_eq!(parse_span_kind(Some("agent")), SpanKind::Agent);
        assert_eq!(parse_span_kind(Some("http")), SpanKind::Http);
        assert_eq!(parse_span_kind(Some("unknown")), SpanKind::Function);
        assert_eq!(parse_span_kind(None), SpanKind::Function);
    }

    #[test]
    fn test_build_span_status() {
        let ok = build_span_status(false, None, None, None);
        assert!(ok.is_ok());

        let err = build_span_status(true, Some("ValueError"), Some("invalid value"), None);
        assert!(err.is_error());

        let err_bt = build_span_status(
            true,
            Some("RuntimeError"),
            Some("failure"),
            Some("File app.py line 42"),
        );
        assert!(err_bt.is_error());
    }
}

