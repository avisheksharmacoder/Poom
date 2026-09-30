import time
from typing import Optional, Dict, Any, Union

try:
    from . import poom_native  # type: ignore
except ImportError:
    try:
        import poom_native  # type: ignore
    except ImportError:
        poom_native = None

_IS_INITIALIZED = False

def init(socket_path: Optional[str] = None, buffer_capacity: int = 65536) -> bool:
    """
    Initializes the Poom client and starts the dedicated background OS worker thread.
    
    Args:
        socket_path: Optional path to the Poom daemon Unix Domain Socket.
                     Defaults to /tmp/poom.sock or $XDG_RUNTIME_DIR/poom/poom.sock.
        buffer_capacity: Maximum capacity of the lock-free crossbeam ring buffer (default: 65,536).
    """
    global _IS_INITIALIZED
    if poom_native is None:
        return False
    res = poom_native.init_tracer(socket_path=socket_path, buffer_capacity=buffer_capacity)
    _IS_INITIALIZED = True
    return res

init_tracer = init

def is_initialized() -> bool:
    """Returns True if the Poom client has been initialized."""
    return _IS_INITIALIZED

def generate_trace_id() -> str:
    """Generates a new 128-bit time-ordered UUIDv7 Trace ID."""
    if poom_native is not None:
        return poom_native.generate_trace_id()
    import uuid
    return str(uuid.uuid4())

def generate_span_id() -> str:
    """Generates a new 128-bit time-ordered UUIDv7 Span ID."""
    if poom_native is not None:
        return poom_native.generate_span_id()
    import uuid
    return str(uuid.uuid4())

def record_span(
    trace_id: str,
    span_id: str,
    name: str,
    start_time_nanos: int,
    parent_span_id: Optional[str] = None,
    kind: Optional[str] = None,
    end_time_nanos: Optional[int] = None,
    is_error: bool = False,
    error_type: Optional[str] = None,
    error_message: Optional[str] = None,
    backtrace: Optional[str] = None,
    attributes: Optional[Dict[str, Any]] = None,
    input_tokens: Optional[int] = None,
    output_tokens: Optional[int] = None,
    cached_tokens: Optional[int] = None,
    cost_usd: Optional[float] = None,
) -> bool:
    """
    Submits a span record to the background worker thread without holding the Python GIL.
    Completes in < 10 microseconds.
    """
    if poom_native is None:
        return False
    return poom_native.record_span(
        trace_id=trace_id,
        span_id=span_id,
        name=name,
        start_time_nanos=start_time_nanos,
        parent_span_id=parent_span_id,
        kind=kind,
        end_time_nanos=end_time_nanos,
        is_error=is_error,
        error_type=error_type,
        error_message=error_message,
        backtrace=backtrace,
        attributes=attributes,
        input_tokens=input_tokens,
        output_tokens=output_tokens,
        cached_tokens=cached_tokens,
        cost_usd=cost_usd,
    )

def evaluate(
    trace_id: str,
    name: str,
    span_id: Optional[str] = None,
    score: Optional[Union[float, bool, str]] = None,
    comment: Optional[str] = None,
) -> bool:
    """
    Records an evaluation score or categorical feedback for a trace or span.
    """
    if poom_native is None:
        return False

    score_float = None
    score_bool = None
    score_text = None

    if isinstance(score, bool):
        score_bool = score
    elif isinstance(score, (int, float)):
        score_float = float(score)
    elif isinstance(score, str):
        score_text = score

    return poom_native.record_evaluation(
        trace_id=trace_id,
        span_id=span_id,
        name=name,
        score_float=score_float,
        score_bool=score_bool,
        score_text=score_text,
        comment=comment,
    )

def flush(timeout_seconds: float = 2.0) -> bool:
    """
    Flushes all queued spans across the IPC boundary up to the specified timeout.
    """
    if poom_native is None:
        return False
    timeout_ms = int(timeout_seconds * 1000)
    return poom_native.flush(timeout_ms=timeout_ms)

def shutdown() -> None:
    """
    Shuts down the background worker thread cleanly.
    """
    global _IS_INITIALIZED
    if poom_native is not None:
        poom_native.shutdown()
    _IS_INITIALIZED = False
