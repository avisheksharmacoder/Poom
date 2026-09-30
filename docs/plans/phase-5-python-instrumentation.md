# Phase 5: PyO3 Native Python Client & FastAPI Instrumentation

## 1. Objective
Build the client-side instrumentation SDK targeting Python and FastAPI. The engine hooks into ASGI pipelines and user functions, providing automated context propagation, sub-100 microsecond overhead, and zero GIL contention.

---

## 2. Architecture & Concurrency Strategy

```
FastAPI ASGI Pipeline / Python Runtime
  │
  ├── ASGI Tracing Middleware (Extracts route, headers, sets root trace context)
  │     │
  │     └── ContextVar Propagation (TraceId, SpanId stack)
  │           │
  │           ├── @trace / @trace_tool Decorators (Sync & Async)
  │           │     │
  │           │     └── PyO3 C-Extension (`poom_native`)
  │           │           │
  │           │           └── Lock-Free Bounded Ring Buffer / Crossbeam Channel
  │           │                 │
  ▼           ▼                 └── Dedicated Background OS Worker Thread
FastAPI Core Coroutines                       │ (Pushes Postcard bytes via UDS / Pipe)
(Zero GIL blocking)                           ▼
                                      /tmp/poom.sock
```

### 2.1 Zero-GIL Ingestion & Background OS Worker
- Python user code captures timestamps and span attributes.
- The PyO3 extension takes the record, immediately releases the Python GIL (`py.allow_threads(...)`), and pushes raw records or serialized postcard frames into a `crossbeam-channel` or lock-free ring buffer.
- The Python call returns in **< 10 microseconds**.
- A dedicated background OS thread (spawned in Rust, completely outside Python's asyncio and GIL) drains the channel, manages the IPC socket connection, and streams byte frames to the daemon.

### 2.2 Backpressure & Safety Policy
- Under extreme traffic spikes, the bounded channel drops spans gracefully (or logs a rate-limit warning) rather than allocating unbounded memory or degrading user API response times.
- If the Poom daemon is shut down, the Python client fails silently or buffers with exponential backoff—user application requests **never** crash or stall.

### 2.3 Context Propagation & Async Handling
- Use Python's native `contextvars.ContextVar`:
  - `current_trace_id: ContextVar[Optional[str]]`
  - `current_span_stack: ContextVar[Tuple[str, ...]]`
- Enables full async task isolation across `asyncio.create_task` and concurrent request handling.
- Child spans automatically identify their immediate parent from `current_span_stack[-1]`.

### 2.4 Developer Ergonomics
- `@trace(name: Optional[str] = None, kind: SpanKind = SpanKind.Function)`: Wraps both synchronous functions and `async def` coroutines. Automatically captures unhandled exceptions and attaches tracebacks.
- `@trace_tool(name: Optional[str] = None)`: Captures tool arguments, schemas, return values, and execution duration.
- `PoomMiddleware`: ASGI middleware wrapping FastAPI / Starlette applications. Automatically records HTTP method, route template, status code, query params, and duration.

---

## 3. Deliverables in `python/`

### `python/poom-py` (PyO3 C-Extension)
- `Cargo.toml`: Configured with `pyo3 = { version = "0.22", features = ["abi3-py39", "extension-module"] }`.
- `src/lib.rs`: Exposes native functions `init_tracer()`, `record_span()`, `flush()`, `shutdown()`.
- `src/worker.rs`: Rust background OS thread managing IPC transmission.

### `python/poom` (Pure Python Wrapper)
- `pyproject.toml`: Configured for `maturin` builds.
- `poom/decorators.py`: `@trace`, `@trace_tool`, context management helpers.
- `poom/middleware.py`: `PoomMiddleware` ASGI implementation.
- `poom/context.py`: `contextvars` stack management.
- `tests/test_fastapi_integration.py`: End-to-end FastAPI test app simulating concurrent requests.

---

## 4. Verification & Acceptance Criteria
- [ ] Client latency overhead is measured at **< 100 microseconds per span** under benchmark.
- [ ] No GIL contention during span submission across 64 concurrent Python async tasks.
- [ ] Zero unhandled Python exceptions raised if the IPC socket is terminated mid-request.
- [ ] Seamless tree hierarchy reconstruction in multi-nested `@trace` async calls.
