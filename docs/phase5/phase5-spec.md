# Phase 5: PyO3 Native Python Client & FastAPI Auto-Instrumentation
## Detailed Architectural & Mechanical Specification

---

## 1. Scope, Purpose & Architectural Boundary

Phase 5 builds the client-side instrumentation SDK targeting Python, FastAPI, and asynchronous ASGI applications. 

### 1.1 The Python GIL & Latency Challenge
Observability libraries written in pure Python often degrade web application performance:
1. **GIL Contention:** Serializing JSON or sending payloads over network sockets inside the Python process locks the Global Interpreter Lock (GIL), delaying concurrent request coroutines.
2. **Async Event Loop Blocking:** Performing synchronous socket writes or blocking I/O inside FastAPI endpoints stalls the asyncio event loop.
3. **Memory Spikes:** Python dictionary allocations for thousands of spans cause memory fragmentation and trigger unpredictable garbage collection (GC) pauses.

### 1.2 The Poom Solution
Poom eliminates these issues by pairing a high-speed **PyO3 native C-extension (`poom_native`)** with a dedicated background OS worker thread:
- Span recording completes in **$< 10$ microseconds** in Python.
- The Python GIL is released immediately (`py.allow_threads`).
- The worker thread streams byte frames directly to `/tmp/poom.sock` over a lock-free queue.
- Python user code never waits on IPC I/O, socket handshakes, or database flushes.

---

## 2. Concurrency Architecture & Zero-GIL Ingestion

```
FastAPI ASGI Pipeline / Python asyncio Runtime
  │
  ├── PoomMiddleware (Extracts route, sets root TraceId & SpanId stack)
  │     │
  │     └── ContextVar Propagation (_CURRENT_TRACE_ID, _SPAN_ID_STACK)
  │           │
  │           ├── @trace / @trace_tool Decorators (Sync & Async)
  │           │     │
  │           │     └── PyO3 C-Extension (poom_native)
  │           │           │
  │           │           ├── Immediate GIL Release (py.allow_threads)
  │           │           │
  │           │           └── Lock-Free Bounded Channel (crossbeam-channel)
  ▼           ▼                       │
FastAPI Core Coroutines               ▼
(Zero GIL blocking)           Dedicated Background OS Worker Thread
                                      │ (Drains batches & encodes Postcard frames)
                                      ▼
                              /tmp/poom.sock (Unix Domain Socket)
```

### 2.1 The Two-Tier Thread Architecture
1. **Tier 1: Python Coroutines (Producer):**
   - User functions decorated with `@trace` capture nanosecond timestamps (`time.time_ns()`), arguments, and attributes.
   - Calls the native function `poom_native.record_span(...)`.
   - The native bridge converts arguments into Rust primitives, releases the GIL, pushes the record onto a lock-free `crossbeam-channel`, and returns control to Python immediately.
2. **Tier 2: Dedicated Background OS Thread (Consumer):**
   - Spawned during `init_tracer()` using `std::thread::spawn`.
   - Runs independently of Python's asyncio event loop and never acquires the GIL.
   - Batches up to 1,000 spans from the channel.
   - Manages the Unix Domain Socket (or Windows Named Pipe) connection to `/tmp/poom.sock`.
   - Serializes batches into `ClientMessage::IngestSpans` postcard frames and pushes bytes over the wire.

### 2.2 Backpressure & Bounded Ring Buffer Policy
- The crossbeam channel is bounded (default capacity: **65,536 spans**).
- If traffic surges beyond socket ingestion capacity, the client uses non-blocking `try_send`.
- If the buffer is full, excess spans are dropped gracefully (or logged via a rate-limited counter) rather than degrading API response times or causing Python Out-Of-Memory (OOM) crashes.
- If the Poom daemon is temporarily offline, the client buffers spans and attempts exponential backoff reconnection without raising unhandled exceptions in user code.

---

## 3. Context Propagation & Async Task Isolation

To accurately reconstruct hierarchical execution trees across concurrent asynchronous tasks, Poom leverages Python's native `contextvars` module.

### 3.1 Context Variables
- **`_CURRENT_TRACE_ID: ContextVar[Optional[str]]`:** Holds the active 128-bit time-ordered TraceId (UUIDv7) for the current task context.
- **`_SPAN_ID_STACK: ContextVar[Tuple[str, ...]]`:** An immutable tuple representing the active call stack of SpanIds.

### 3.2 Hierarchy Resolution Workflow
1. **Root Boundary Entry:**
   - When an incoming HTTP request hits `PoomMiddleware`, it checks `_CURRENT_TRACE_ID`.
   - If empty, it generates a new UUIDv7 TraceId.
   - Creates the root HTTP span (`SpanKind.Http`) with `parent_span_id = None`.
   - Pushes root `SpanId` onto `_SPAN_ID_STACK`.
2. **Nested Function / Tool Calls:**
   - When a decorated function (e.g. `@trace(kind=SpanKind.Agent)`) is called, it inspects `_SPAN_ID_STACK`.
   - The top element of the tuple becomes the child span's `parent_span_id`.
   - Generates a new `SpanId`, appends it to `_SPAN_ID_STACK`, and executes the function.
3. **Sub-Task & Parallel Execution Isolation:**
   - Because `contextvars` copy their context to newly spawned asyncio tasks (`asyncio.create_task` or `asyncio.gather`), concurrent branches inherit the parent span ID correctly without race conditions or thread-local bleeding.
4. **Boundary Exit:**
   - In a `finally` block, the span pops its ID from `_SPAN_ID_STACK` and records completion time.

---

## 4. Developer Ergonomics & Decorator Suite

### 4.1 `@trace(name=None, kind=SpanKind.Function)`
- Universal decorator supporting both standard synchronous functions (`def`) and asynchronous coroutines (`async def`).
- Uses `functools.wraps` to preserve function signatures, type annotations, and docstrings.
- **Exception Trapping:** Automatically catches any unhandled exception, marks span status as `Error`, records the exception class name, error message, and full traceback (`traceback.format_exc()`), then re-raises the exception so application error handling remains untouched.

### 4.2 `@trace_tool(name=None)`
- Specialized decorator tailored for LLM tool calling and agent function invocations (`SpanKind.Tool`).
- Automatically serializes function arguments as JSON/attributes.
- Records return values and execution latency for model tool-use evaluation.

### 4.3 `PoomMiddleware` (ASGI Middleware)
- Wraps FastAPI, Starlette, or raw ASGI applications.
- Auto-instruments incoming HTTP routes:
  - Captures HTTP method (`GET`, `POST`, `PUT`, etc.).
  - Extracts parameterized route templates (`/api/v1/users/{id}`) instead of noisy raw paths.
  - Records query parameters, client IP, user agent, and response status codes (`200`, `404`, `500`).
  - Sets the root execution boundary for the entire request lifecycle.

---

## 5. Crate & Package Topology

The Python ecosystem is organized into two components:

```
poom/python/
├── poom-py/                         # PyO3 C-Extension (Rust)
│   ├── Cargo.toml                   # crate-type = ["cdylib"], pyo3 = "0.22"
│   └── src/
│       ├── lib.rs                   # PyO3 module definition & native bindings
│       ├── worker.rs                # Standalone background OS thread & UDS stream
│       └── bridge.rs                # Conversion between Python objects and poom-types
│
└── poom/                            # Pure Python distribution package
    ├── pyproject.toml               # Maturin packaging configuration
    └── poom/
        ├── __init__.py              # Public exports (@trace, @trace_tool, PoomMiddleware)
        ├── context.py               # ContextVar trace & span hierarchy stack
        ├── decorators.py            # Universal sync & async function decorators
        ├── middleware.py            # FastAPI / Starlette ASGI middleware
        └── models.py                # Python enum mirrors (SpanKind, SpanStatus)
```

### 5.1 PyO3 Stable ABI (`abi3`)
- `poom-py` targets the Python Stable ABI (`features = ["abi3-py39"]`).
- A single compiled wheel functions across **Python 3.9, 3.10, 3.11, 3.12, and 3.13** without requiring separate builds per minor Python release.

---

## 6. Verification, Integration & Benchmarking Plan

### 6.1 Verification Tests
1. **Sync & Async Decorator Roundtrip:**
   - Execute nested synchronous and asynchronous functions decorated with `@trace`.
   - Verify parent-child hierarchy is preserved accurately across 5 levels of nesting.
2. **FastAPI End-to-End Test:**
   - Spin up a real FastAPI test app wrapped with `PoomMiddleware`.
   - Call endpoints simulating LLM calls, tool executions, and exception routes.
   - Verify all spans arrive in the local Poom daemon with valid HTTP routes, status codes, and tracebacks.
3. **Zero-GIL Overhead Benchmark:**
   - Run 64 concurrent Python tasks submitting 10,000 spans.
   - Measure client-side submission latency: must complete in **$< 100$ microseconds** per span.
4. **Daemon Disconnect Resilience:**
   - Shut down the Poom daemon while a FastAPI test app is handling requests.
   - Verify that endpoints continue serving HTTP 200 responses without hanging or crashing.
