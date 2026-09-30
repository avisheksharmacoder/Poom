"""
Poom: Pure-Rust, Local-First, Low-Overhead LLM & Application Tracing
"""

from .client import (
    evaluate,
    flush,
    generate_span_id,
    generate_trace_id,
    init,
    init_tracer,
    is_initialized,
    record_span,
    shutdown,
)
from .context import (
    SpanContext,
    get_current_parent_span_id,
    get_current_span_context,
    get_current_trace_id,
    push_span_id,
    reset_span_id,
    reset_trace_id,
    set_attribute,
    set_current_trace_id,
    set_tokens,
)
from .decorators import span, trace, trace_tool
from .middleware import PoomMiddleware
from .models import SpanKind, SpanMetrics, SpanStatus

__version__ = "0.1.0"

__all__ = [
    "init",
    "init_tracer",
    "flush",
    "shutdown",
    "evaluate",
    "span",
    "trace",
    "trace_tool",
    "set_attribute",
    "set_tokens",
    "SpanContext",
    "PoomMiddleware",
    "SpanKind",
    "SpanStatus",
    "SpanMetrics",
    "get_current_trace_id",
    "get_current_parent_span_id",
    "get_current_span_context",
    "generate_trace_id",
    "generate_span_id",
    "is_initialized",
]
