import functools
import inspect
import time
import traceback
from typing import Any, Callable, Dict, Optional, Union

from .client import (
    generate_span_id,
    generate_trace_id,
    record_span,
)
from .context import (
    SpanContext,
    get_current_parent_span_id,
    get_current_trace_id,
    push_span_id,
    reset_span_context,
    reset_span_id,
    reset_trace_id,
    set_current_span_context,
    set_current_trace_id,
)
from .models import SpanKind

class span:
    """
    Context manager supporting both synchronous (`with`) and asynchronous (`async with`) blocks.
    
    Usage:
        with poom.span("retrieve_docs", kind=SpanKind.TOOL) as s:
            s.set_attribute("query", "rag architecture")
            docs = fetch_docs()
            s.set_tokens(input_tokens=100)
    """

    def __init__(
        self,
        name: str,
        kind: Union[SpanKind, str] = SpanKind.FUNCTION,
        attributes: Optional[Dict[str, Any]] = None,
    ) -> None:
        self.name = name
        self.kind = kind.value if isinstance(kind, SpanKind) else str(kind)
        self.attributes = attributes
        self.trace_id: Optional[str] = None
        self.span_id: Optional[str] = None
        self.parent_span_id: Optional[str] = None
        self.start_nanos: int = 0
        self.context: Optional[SpanContext] = None
        self._trace_token = None
        self._span_token = None
        self._ctx_token = None

    def _enter(self) -> SpanContext:
        self.trace_id = get_current_trace_id()
        if self.trace_id is None:
            self.trace_id = generate_trace_id()
            self._trace_token = set_current_trace_id(self.trace_id)

        self.parent_span_id = get_current_parent_span_id()
        self.span_id = generate_span_id()
        self._span_token = push_span_id(self.span_id)

        self.context = SpanContext(
            trace_id=self.trace_id,
            span_id=self.span_id,
            initial_attrs=self.attributes,
        )
        self._ctx_token = set_current_span_context(self.context)
        self.start_nanos = time.time_ns()
        return self.context

    def _exit(self, exc_type, exc_val, exc_tb) -> None:
        end_nanos = time.time_ns()
        is_error = exc_type is not None
        err_type = exc_type.__name__ if exc_type else None
        err_msg = str(exc_val) if exc_val else None
        bt = traceback.format_exc() if is_error else None

        record_span(
            trace_id=self.trace_id,
            span_id=self.span_id,
            parent_span_id=self.parent_span_id,
            name=self.name,
            kind=self.kind,
            start_time_nanos=self.start_nanos,
            end_time_nanos=end_nanos,
            is_error=is_error,
            error_type=err_type,
            error_message=err_msg,
            backtrace=bt,
            attributes=self.context.attributes if self.context else None,
            input_tokens=self.context.input_tokens if self.context else None,
            output_tokens=self.context.output_tokens if self.context else None,
            cached_tokens=self.context.cached_tokens if self.context else None,
            cost_usd=self.context.cost_usd if self.context else None,
        )

        if self._ctx_token is not None:
            reset_span_context(self._ctx_token)
        if self._span_token is not None:
            reset_span_id(self._span_token)
        if self._trace_token is not None:
            reset_trace_id(self._trace_token)

    def __enter__(self) -> SpanContext:
        return self._enter()

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self._exit(exc_type, exc_val, exc_tb)

    async def __aenter__(self) -> SpanContext:
        return self._enter()

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        self._exit(exc_type, exc_val, exc_tb)

def trace(
    name: Optional[str] = None,
    kind: Union[SpanKind, str] = SpanKind.FUNCTION,
    attributes: Optional[Dict[str, Any]] = None,
) -> Callable:
    """
    Universal decorator instrumenting synchronous and asynchronous functions.
    Preserves context hierarchy across concurrent asyncio tasks using contextvars.
    Supports dynamic attribute and token injection via `poom.set_attribute` and `poom.set_tokens`.
    Captures unhandled exceptions and full tracebacks without altering control flow.
    """
    kind_str = kind.value if isinstance(kind, SpanKind) else str(kind)

    def decorator(func: Callable) -> Callable:
        span_name = name or func.__name__

        if inspect.iscoroutinefunction(func):
            @functools.wraps(func)
            async def async_wrapper(*args: Any, **kwargs: Any) -> Any:
                with span(name=span_name, kind=kind_str, attributes=attributes):
                    return await func(*args, **kwargs)

            return async_wrapper
        else:
            @functools.wraps(func)
            def sync_wrapper(*args: Any, **kwargs: Any) -> Any:
                with span(name=span_name, kind=kind_str, attributes=attributes):
                    return func(*args, **kwargs)

            return sync_wrapper

    return decorator

def trace_tool(name: Optional[str] = None) -> Callable:
    """
    Specialized decorator tailored for agent tool calls and function calling.
    Automatically captures input parameters and return values into span attributes.
    """
    def decorator(func: Callable) -> Callable:
        tool_name = name or func.__name__

        def _build_input_attrs(args: tuple, kwargs: dict) -> Dict[str, Any]:
            sig = inspect.signature(func)
            bound = sig.bind_partial(*args, **kwargs)
            bound.apply_defaults()
            inputs = {}
            for k, v in bound.arguments.items():
                try:
                    inputs[k] = str(v)
                except Exception:
                    inputs[k] = "<unserializable>"
            return {"tool.name": tool_name, "tool.inputs": inputs}

        if inspect.iscoroutinefunction(func):
            @functools.wraps(func)
            async def async_tool_wrapper(*args: Any, **kwargs: Any) -> Any:
                attrs = _build_input_attrs(args, kwargs)
                with span(name=tool_name, kind=SpanKind.TOOL, attributes=attrs) as s:
                    result = await func(*args, **kwargs)
                    try:
                        s.set_attribute("tool.output", str(result))
                    except Exception:
                        pass
                    return result

            return async_tool_wrapper
        else:
            @functools.wraps(func)
            def sync_tool_wrapper(*args: Any, **kwargs: Any) -> Any:
                attrs = _build_input_attrs(args, kwargs)
                with span(name=tool_name, kind=SpanKind.TOOL, attributes=attrs) as s:
                    result = func(*args, **kwargs)
                    try:
                        s.set_attribute("tool.output", str(result))
                    except Exception:
                        pass
                    return result

            return sync_tool_wrapper

    return decorator
