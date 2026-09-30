# Phase 2: Cross-Platform IPC & Transport Engine

## 1. Objective
Build an ultra-fast, cross-platform Local Inter-Process Communication (IPC) transport layer that connects instrumented worker processes (FastAPI/Python, etc.) to the local Poom daemon with minimal CPU cycles and zero network stack overhead.

---

## 2. Architecture & Design

### 2.1 Transport Channels
- **Linux & macOS:** Unix Domain Socket (UDS) at `/tmp/poom.sock` (or `$XDG_RUNTIME_DIR/poom.sock`).
- **Windows:** Named Pipes at `\\.\pipe\poom`.
- Eliminates TCP handshake overhead, port collisions, loopback proxy interference, and OS firewall prompts.

### 2.2 Streaming & Framing via `tokio-util`
- Implements an asynchronous framed transport using `tokio_util::codec::Framed` and custom `Encoder`/`Decoder` implementations or `LengthDelimitedCodec`.
- Multiplexes multiple concurrent client connections (e.g. 16 Uvicorn/Gunicorn worker processes streaming spans simultaneously).

### 2.3 Connection Lifecycle & Resilience
- **Server Side:**
  - Creates the socket file or named pipe.
  - Cleans up stale socket files gracefully on startup and shutdown.
  - Configures socket permissions (mode `0600` on Unix for local security).
  - Handles client disconnects and incomplete frames without thread panics.
- **Client Side:**
  - Non-blocking asynchronous connect with retry and exponential backoff.
  - Reconnect loop: if the daemon restarts, the client attempts seamless reconnection.
  - Fail-safe fallback: If the socket is unreachable, the client transitions to a dropping or buffering state without halting application processes.

---

## 3. Deliverables in `crates/poom-transport`

- `uds.rs`: Unix domain socket server and client abstractions.
- `named_pipe.rs`: Windows named pipe server and client abstractions.
- `listener.rs`: Generic `IpcListener` trait unifying Unix/Windows implementations.
- `stream.rs`: Generic `IpcStream` trait with Tokio `AsyncRead` and `AsyncWrite`.
- `codec.rs`: Framed message codec wrapping `poom-protocol`.
- `tests/transport_smoke.rs`: Multi-client concurrent transmission integration tests.

---

## 4. Verification & Acceptance Criteria
- [ ] Concurrently supports 32 concurrent client connections streaming at full speed.
- [ ] Graceful cleanup of `/tmp/poom.sock` on `SIGINT` / `SIGTERM`.
- [ ] Benchmarked throughput: Transmits > 100,000 raw frames/second across the IPC boundary on standard developer hardware.
- [ ] Handles sudden client disconnects mid-frame without crashing the listener.
