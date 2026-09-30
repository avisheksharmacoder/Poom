import contextvars
from typing import Any, Dict, Optional, Tuple

# Context variable tracking the current trace ID (UUIDv7 string) across asyncio tasks
_CURRENT_TRACE_ID: contextvars.ContextVar[Optional[str]] = contextvars.ContextVar(
    "_CURRENT_TRACE_ID", default=None
)

# Context variable tracking the stack of span IDs (top of tuple is current parent)
_SPAN_ID_STACK: contextvars.ContextVar[Tuple[str, ...]] = contextvars.ContextVar(
    "_SPAN_ID_STACK", default=()
)

class SpanContext:
    """Active span metadata holder allowing dynamic runtime updates to attributes and token metrics."""

    def __init__(
        self,
        trace_id: str,
        span_id: str,
        initial_attrs: Optional[Dict[str, Any]] = None,
    ) -> None:
        self.trace_id = trace_id
        self.span_id = span_id
        self.attributes: Dict[str, Any] = dict(initial_attrs) if initial_attrs else {}
        self.input_tokens: Optional[int] = None
        self.output_tokens: Optional[int] = None
        self.cached_tokens: Optional[int] = None
        self.cost_usd: Optional[float] = None

    def set_attribute(self, key: str, value: Any) -> "SpanContext":
        """Sets an arbitrary metadata attribute on this span."""
        self.attributes[key] = value
        return self

    def set_tokens(
        self,
        input_tokens: Optional[int] = None,
        output_tokens: Optional[int] = None,
        cached_tokens: Optional[int] = None,
        cost_usd: Optional[float] = None,
    ) -> "SpanContext":
        """Sets token counts and cost metrics for LLM calls."""
        if input_tokens is not None:
            self.input_tokens = input_tokens
        if output_tokens is not None:
            self.output_tokens = output_tokens
        if cached_tokens is not None:
            self.cached_tokens = cached_tokens
        if cost_usd is not None:
            self.cost_usd = cost_usd
        return self

# Context variable tracking the active span context for dynamic attribute/token injection
_CURRENT_SPAN_CONTEXT: contextvars.ContextVar[Optional[SpanContext]] = contextvars.ContextVar(
    "_CURRENT_SPAN_CONTEXT", default=None
)

def get_current_trace_id() -> Optional[str]:
    """Returns the active trace ID for the current async task context."""
    return _CURRENT_TRACE_ID.get()

def set_current_trace_id(trace_id: str) -> contextvars.Token:
    """Sets the active trace ID, returning a reset token."""
    return _CURRENT_TRACE_ID.set(trace_id)

def reset_trace_id(token: contextvars.Token) -> None:
    """Restores the previous trace ID using the provided token."""
    _CURRENT_TRACE_ID.reset(token)

def get_current_parent_span_id() -> Optional[str]:
    """Returns the active parent span ID (top of the span stack) or None."""
    stack = _SPAN_ID_STACK.get()
    return stack[-1] if stack else None

def push_span_id(span_id: str) -> contextvars.Token:
    """Pushes a new span ID onto the stack, returning a reset token."""
    current = _SPAN_ID_STACK.get()
    return _SPAN_ID_STACK.set(current + (span_id,))

def reset_span_id(token: contextvars.Token) -> None:
    """Restores the previous span ID stack using the provided token."""
    _SPAN_ID_STACK.reset(token)

def get_current_span_context() -> Optional[SpanContext]:
    """Returns the active SpanContext for the current execution context."""
    return _CURRENT_SPAN_CONTEXT.get()

def set_current_span_context(ctx: SpanContext) -> contextvars.Token:
    """Sets the active SpanContext, returning a reset token."""
    return _CURRENT_SPAN_CONTEXT.set(ctx)

def reset_span_context(token: contextvars.Token) -> None:
    """Restores the previous SpanContext using the provided token."""
    _CURRENT_SPAN_CONTEXT.reset(token)

def set_attribute(key: str, value: Any) -> bool:
    """
    Sets a dynamic attribute on the currently executing span.
    Returns True if an active span was found, False otherwise.
    """
    ctx = _CURRENT_SPAN_CONTEXT.get()
    if ctx is not None:
        ctx.set_attribute(key, value)
        return True
    return False

def set_tokens(
    input_tokens: Optional[int] = None,
    output_tokens: Optional[int] = None,
    cached_tokens: Optional[int] = None,
    cost_usd: Optional[float] = None,
) -> bool:
    """
    Sets token counts and cost metrics on the currently executing span.
    Returns True if an active span was found, False otherwise.
    """
    ctx = _CURRENT_SPAN_CONTEXT.get()
    if ctx is not None:
        ctx.set_tokens(
            input_tokens=input_tokens,
            output_tokens=output_tokens,
            cached_tokens=cached_tokens,
            cost_usd=cost_usd,
        )
        return True
    return False
