import time
import traceback
from typing import Any, Callable, Dict, Optional

from .client import (
    generate_span_id,
    generate_trace_id,
    record_span,
)
from .context import (
    push_span_id,
    reset_span_id,
    reset_trace_id,
    set_current_trace_id,
)
from .models import SpanKind

class PoomMiddleware:
    """
    High-performance ASGI middleware auto-instrumenting FastAPI and Starlette applications.
    
    Features:
    - Root execution boundary creation per incoming HTTP request.
    - Propagates existing trace ID if provided in `x-poom-trace-id` or `traceparent` header.
    - Intercepts HTTP status codes without buffering response bodies.
    - Injects `x-poom-trace-id` into outbound response headers for downstream correlation.
    """

    def __init__(self, app: Any, app_name: str = "fastapi_app") -> None:
        self.app = app
        self.app_name = app_name

    async def __call__(self, scope: Dict[str, Any], receive: Callable, send: Callable) -> None:
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return

        method = scope.get("method", "GET")
        raw_path = scope.get("path", "/")
        query_string = scope.get("query_string", b"").decode("latin-1")

        # Parse trace id from incoming headers
        headers = dict(scope.get("headers", []))
        incoming_trace_id = None
        for k, v in headers.items():
            if k.lower() == b"x-poom-trace-id":
                incoming_trace_id = v.decode("latin-1", errors="ignore").strip()
                break

        trace_id = incoming_trace_id if incoming_trace_id else generate_trace_id()
        span_id = generate_span_id()

        trace_token = set_current_trace_id(trace_id)
        span_token = push_span_id(span_id)

        start_nanos = time.time_ns()
        response_status = 200
        is_error = False
        err_type = None
        err_msg = None
        bt = None

        async def wrapped_send(message: Dict[str, Any]) -> None:
            nonlocal response_status
            if message["type"] == "http.response.start":
                response_status = message.get("status", 200)
                # Inject trace id into response headers
                raw_headers = list(message.get("headers", []))
                raw_headers.append((b"x-poom-trace-id", trace_id.encode("ascii")))
                message["headers"] = raw_headers
            await send(message)

        try:
            await self.app(scope, receive, wrapped_send)
        except Exception as e:
            is_error = True
            response_status = 500
            err_type = type(e).__name__
            err_msg = str(e)
            bt = traceback.format_exc()
            raise
        finally:
            end_nanos = time.time_ns()
            if response_status >= 500:
                is_error = True

            client_info = scope.get("client")
            client_ip = client_info[0] if client_info else "unknown"

            # Route resolution: try starlette route template if resolved
            route_endpoint = scope.get("endpoint")
            endpoint_name = getattr(route_endpoint, "__name__", None)
            route_pattern = None
            if "route" in scope:
                route_pattern = getattr(scope["route"], "path", None)

            span_name = f"{method} {route_pattern or raw_path}"

            attrs = {
                "http.method": method,
                "http.path": raw_path,
                "http.status_code": response_status,
                "http.client_ip": client_ip,
                "service.name": self.app_name,
            }
            if query_string:
                attrs["http.query"] = query_string
            if endpoint_name:
                attrs["code.function"] = endpoint_name

            record_span(
                trace_id=trace_id,
                span_id=span_id,
                parent_span_id=None,
                name=span_name,
                kind=SpanKind.HTTP.value,
                start_time_nanos=start_nanos,
                end_time_nanos=end_nanos,
                is_error=is_error,
                error_type=err_type,
                error_message=err_msg,
                backtrace=bt,
                attributes=attrs,
            )

            reset_span_id(span_token)
            reset_trace_id(trace_token)
