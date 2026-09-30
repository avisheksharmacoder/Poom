pub mod bridge;
pub mod worker;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use smol_str::SmolStr;

use poom_types::{
    EvaluationRecord, EvaluationValue, SpanId, SpanMetrics, SpanRecord, TraceId,
};
use std::sync::{Arc, RwLock};
use bridge::{build_span_status, parse_span_kind, py_to_attribute_value};
use worker::BackgroundWorker;

static WORKER: RwLock<Option<Arc<BackgroundWorker>>> = RwLock::new(None);

fn get_or_init_worker() -> Arc<BackgroundWorker> {
    {
        if let Ok(r) = WORKER.read() {
            if let Some(ref w) = *r {
                return Arc::clone(w);
            }
        }
    }
    let mut w = WORKER.write().unwrap();
    if let Some(ref existing) = *w {
        return Arc::clone(existing);
    }
    let worker = Arc::new(BackgroundWorker::start(None, 65_536));
    *w = Some(Arc::clone(&worker));
    worker
}

/// Initializes the background worker with custom socket paths and queue capacities.
#[pyfunction]
#[pyo3(signature = (socket_path = None, buffer_capacity = None))]
pub fn init_tracer(socket_path: Option<String>, buffer_capacity: Option<usize>) -> PyResult<bool> {
    let capacity = buffer_capacity.unwrap_or(65_536);
    let path = socket_path.map(PathBuf::from);

    let mut w = WORKER.write().unwrap();
    if let Some(ref old_worker) = *w {
        old_worker.shutdown();
    }
    *w = Some(Arc::new(BackgroundWorker::start(path, capacity)));
    Ok(true)
}

/// Generates a time-ordered 128-bit UUIDv7 TraceId as a standard hex string.
#[pyfunction]
pub fn generate_trace_id() -> String {
    TraceId::generate().to_string()
}

/// Generates a time-ordered 128-bit UUIDv7 SpanId as a standard hex string.
#[pyfunction]
pub fn generate_span_id() -> String {
    SpanId::generate().to_string()
}

/// Dispatches an execution span directly to the lock-free background queue.
///
/// Immediately releases the Python GIL (`py.allow_threads`) so user application
/// coroutines return in < 10 microseconds with zero GIL blocking.
#[pyfunction]
#[pyo3(signature = (
    trace_id,
    span_id,
    name,
    start_time_nanos,
    parent_span_id = None,
    kind = None,
    end_time_nanos = None,
    is_error = false,
    error_type = None,
    error_message = None,
    backtrace = None,
    attributes = None,
    input_tokens = None,
    output_tokens = None,
    cached_tokens = None,
    cost_usd = None
))]
pub fn record_span(
    py: Python<'_>,
    trace_id: &str,
    span_id: &str,
    name: &str,
    start_time_nanos: u64,
    parent_span_id: Option<&str>,
    kind: Option<&str>,
    end_time_nanos: Option<u64>,
    is_error: bool,
    error_type: Option<&str>,
    error_message: Option<&str>,
    backtrace: Option<&str>,
    attributes: Option<&Bound<'_, PyDict>>,
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
    cached_tokens: Option<u32>,
    cost_usd: Option<f64>,
) -> PyResult<bool> {
    let t_id = TraceId::from_str(trace_id).unwrap_or_else(|_| TraceId::generate());
    let s_id = SpanId::from_str(span_id).unwrap_or_else(|_| SpanId::generate());
    let p_id = parent_span_id.and_then(|p| SpanId::from_str(p).ok());

    let span_kind = parse_span_kind(kind);
    let status = build_span_status(is_error, error_type, error_message, backtrace);

    // Extract attributes
    let mut attr_map = BTreeMap::new();
    if let Some(dict) = attributes {
        for (k, v) in dict {
            if let Ok(key_str) = k.extract::<String>() {
                attr_map.insert(SmolStr::new(key_str), py_to_attribute_value(&v));
            }
        }
    }

    let mut metrics = SpanMetrics::new();
    if let (Some(inp), Some(out)) = (input_tokens, output_tokens) {
        metrics = metrics.with_tokens(inp, out);
    } else {
        metrics.input_tokens = input_tokens;
        metrics.output_tokens = output_tokens;
    }
    if let Some(cached) = cached_tokens {
        metrics = metrics.with_cached_tokens(cached);
    }
    if let Some(cost) = cost_usd {
        metrics = metrics.with_cost(cost);
    }

    let span = SpanRecord {
        trace_id: t_id,
        span_id: s_id,
        parent_span_id: p_id,
        name: SmolStr::new(name),
        kind: span_kind,
        start_time_unix_nanos: start_time_nanos,
        end_time_unix_nanos: end_time_nanos,
        status,
        attributes: attr_map,
        events: Vec::new(),
        metrics,
    };

    let worker = get_or_init_worker();

    // Release the Python GIL and submit to the ring buffer
    let accepted = py.allow_threads(move || worker.submit_span(span));

    Ok(accepted)
}

/// Dispatches an evaluation assessment or feedback score to the background queue.
#[pyfunction]
#[pyo3(signature = (
    trace_id,
    span_id,
    name,
    score_float = None,
    score_bool = None,
    score_text = None,
    comment = None,
    timestamp_nanos = None
))]
pub fn record_evaluation(
    py: Python<'_>,
    trace_id: &str,
    span_id: Option<&str>,
    name: &str,
    score_float: Option<f64>,
    score_bool: Option<bool>,
    score_text: Option<&str>,
    comment: Option<&str>,
    timestamp_nanos: Option<u64>,
) -> PyResult<bool> {
    let t_id = TraceId::from_str(trace_id).unwrap_or_else(|_| TraceId::generate());
    let s_id = span_id.and_then(|s| SpanId::from_str(s).ok());

    let value = if let Some(f) = score_float {
        EvaluationValue::Numeric(f)
    } else if let Some(b) = score_bool {
        EvaluationValue::Boolean(b)
    } else if let Some(t) = score_text {
        EvaluationValue::Categorical(SmolStr::new(t))
    } else {
        EvaluationValue::Numeric(1.0)
    };

    let nanos = timestamp_nanos.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    });

    let eval = EvaluationRecord {
        evaluation_id: SpanId::generate(),
        trace_id: t_id,
        span_id: s_id,
        name: SmolStr::new(name),
        value,
        comment: comment.map(|s| s.to_string()),
        timestamp_unix_nanos: nanos,
    };

    let worker = get_or_init_worker();
    let accepted = py.allow_threads(move || worker.submit_evaluation(eval));

    Ok(accepted)
}

/// Flushes in-flight spans across the IPC boundary up to the specified timeout in milliseconds.
#[pyfunction]
#[pyo3(signature = (timeout_ms = None))]
pub fn flush(py: Python<'_>, timeout_ms: Option<u64>) -> PyResult<bool> {
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(2_000));
    let worker = get_or_init_worker();
    let res = py.allow_threads(move || worker.flush(timeout));
    Ok(res)
}

/// Shuts down the background worker thread.
#[pyfunction]
pub fn shutdown() -> PyResult<()> {
    let mut w = WORKER.write().unwrap();
    if let Some(ref old_worker) = *w {
        old_worker.shutdown();
    }
    *w = None;
    Ok(())
}

/// Native module entrypoint.
#[pymodule]
fn poom_native(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(init_tracer, m)?)?;
    m.add_function(wrap_pyfunction!(generate_trace_id, m)?)?;
    m.add_function(wrap_pyfunction!(generate_span_id, m)?)?;
    m.add_function(wrap_pyfunction!(record_span, m)?)?;
    m.add_function(wrap_pyfunction!(record_evaluation, m)?)?;
    m.add_function(wrap_pyfunction!(flush, m)?)?;
    m.add_function(wrap_pyfunction!(shutdown, m)?)?;
    Ok(())
}
