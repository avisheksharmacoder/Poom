# Phase 2: Cross-Platform IPC & Transport Engine
## Detailed Architectural & Mechanical Specification

---

## 1. Scope, Purpose & Architectural Boundary

Phase 2 establishes the physical high-performance communication pipeline connecting instrumented applications (Python/FastAPI, CLI tools, etc.) to the local Poom background daemon.

### 1.1 Why IPC Over Loopback TCP?
Traditional observability tools connect over local HTTP/TCP (`http://127.0.0.1:4318` or `localhost:8000`). While simple, loopback TCP introduces severe drawbacks:
1. **TCP Handshake & Port Clashes:** Requires dynamic or fixed port bindings (e.g. 4318), leading to port collision errors when multiple instances run or if firewall policies block loopback ports.
2. **OS Network Stack Overhead:** Every packet traverses the TCP/IP stack, calculating checksums, TCP windowing, congestion control, and ACK packets.
3. **Security Vulnerabilities:** Any local process on the machine can probe an open TCP port on `127.0.0.1`.
4. **IPC Superiority:** Unix Domain Sockets (UDS) on Linux/macOS and Named Pipes on Windows operate purely in OS kernel memory via fast buffer copies. They enforce strict filesystem-level permissions (`0600`), eliminate network stack processing, and reduce latency to the bare minimum.

---

## 2. Abstract Transport Trait Architecture

To ensure the upstream ingestion daemon (`poom-daemon`) and client libraries (`poom-py`) remain 100% agnostic to the host operating system, Phase 2 designs a unified, zero-cost transport abstraction layer.

### 2.1 The Unified Stream & Listener Model
- **`IpcStream` Trait / Type:**
  - On Unix: Wraps `tokio::net::UnixStream`.
  - On Windows: Wraps `tokio::net::windows::named_pipe::NamedPipeClient` / `NamedPipeServer`.
  - Implements Tokio's asynchronous I/O primitives: `AsyncRead`, `AsyncWrite`, `Send`, and `Unpin`.
- **`IpcListener` Trait / Type:**
  - On Unix: Wraps `tokio::net::UnixListener`.
  - On Windows: Manages a pool of `NamedPipeServer` instances.
  - Exposes an asynchronous `accept()` method yielding an `IpcStream` and a caller credential struct (e.g., client PID, user ID).

---

## 3. Platform-Specific Mechanics

### 3.1 Unix Domain Sockets (Linux & macOS)

#### Socket Path Resolution Hierarchy:
1. Custom explicit path if provided via configuration or environment variable (`POOM_SOCKET_PATH`).
2. `$XDG_RUNTIME_DIR/poom/poom.sock` (modern Linux standards, stored on RAM-backed `tmpfs` ensuring maximum speed and automatic per-user isolation).
3. Fallback: `/tmp/poom.sock` (with user UID suffix `/tmp/poom-<uid>.sock` to prevent multi-user permission collisions).

#### The Stale Socket Race Condition & Probe Algorithm:
When a daemon crashes or is terminated abruptly (`SIGKILL`), the `.sock` file remains on disk. A naive `bind()` fails with `Address already in use` (`EADDRINUSE`).

Poom resolves this using a **Non-Blocking Connect Probe**:
```
Daemon Startup
  │
  ├─ Check if socket file exists on disk
  │    │
  │    ├─ No: Proceed to bind socket directly
  │    │
  │    └─ Yes: Attempt non-blocking connect() to the existing path
  │         │
  │         ├─ Connect Succeeds: Another active Poom daemon is running!
  │         │  └─ Log error and exit cleanly (or attach as client)
  │         │
  │         └─ Connect Fails (ECONNREFUSED / ENOENT):
  │            ├─ The file is an abandoned stale socket from a past crash.
  │            ├─ Safely delete the stale file via fs::remove_file().
  │            └─ Proceed to bind socket anew.
```

#### Security & POSIX Permissions:
- Immediately following `bind()`, the daemon sets file permissions to `0600` (`S_IRUSR | S_IWUSR`) using `std::os::unix::fs::PermissionsExt`.
- Only processes running under the same user UID can read or write to the socket.
- Other local unprivileged users cannot observe or inject spans.

#### Clean Teardown & Signal Trapping:
- The daemon registers `SIGINT` and `SIGTERM` listeners using `tokio::signal`.
- On shutdown, the accept loop exits cleanly, flushes in-flight connections, and deletes the socket file from the filesystem.

---

### 3.2 Windows Named Pipes

#### Pipe Naming & User Isolation:
- Standard pipe path: `\\.\pipe\poom` (or `\\.\pipe\poom-<username>` in multi-user terminal services).
- Named pipes in Windows reside in the special Named Pipe File System (`NPFS`).

#### The Server Multi-Instance Model:
Unlike Unix sockets where one listener queues incoming connections, Windows Named Pipes require the server to create an explicit pipe instance for each connecting client:
1. The listener initializes the first server instance with `ServerOptions::first_pipe_instance(true)`.
2. As soon as a client connects to the active instance, the listener immediately creates a *new* pipe instance to listen for the next incoming client.
3. Configures 64 KB kernel input and output buffer sizes to support high-throughput span batches.

#### Security & Access Control:
- Configures security attributes restricting pipe access to the current Windows logon session or LocalSystem, preventing cross-session privilege escalation.

---

## 4. Framed Streaming & Codec Integration (`tokio-util`)

Phase 2 bridges the low-level byte streams (`AsyncRead`/`AsyncWrite`) with Phase 1's binary framing protocol (`poom-protocol`) using `tokio_util::codec::Framed`.

### 4.1 Bi-Directional Asynchronous Codec
We implement `tokio_util::codec::Encoder` and `tokio_util::codec::Decoder`:
- **For Inbound Client Streams (`ClientFramed`):**
  - **Decoder:** Reads incoming bytes from `IpcStream`, utilizes Phase 1's `StreamFrameDecoder` to inspect headers, buffer partial reads, and emit fully deserialized `ClientMessage` items.
  - **Encoder:** Takes outgoing `ServerMessage` items (e.g. `HandshakeAck`, `IngestAck`), serializes them via `FrameEncoder`, and pushes framed bytes onto the wire.
- **For Outbound Server Streams (`ServerFramed`):**
  - Symmetric counterpart used by client libraries.

### 4.2 Zero-Allocation Sliding Windows:
- Incoming frames are parsed directly out of `tokio_util`'s reusable `BytesMut` buffer.
- When an application pushes high-frequency span batches, the memory buffer expands to hold the batch and is recycled for subsequent reads, achieving near-zero heap allocations per frame.

---

## 5. Connection Lifecycle, Resilience & Backpressure

### 5.1 Client Reconnection Engine
The client transport worker must never crash or block the host application (e.g. FastAPI ASGI loop) if the local daemon restarts or is temporarily offline.

- **Non-Blocking Connect with Timeout:** Socket connection attempts are bounded by a strict timeout (e.g. 250 milliseconds).
- **Exponential Backoff with Jitter:**
  - Initial retry delay: 50 ms.
  - Multiplier: 2.0x up to a maximum cap of 2,000 ms.
  - Random jitter ($\pm 20\%$): Prevents the "thundering herd" problem where 64 Uvicorn worker processes reconnect simultaneously when the daemon restarts.
- **Graceful Buffering & Fallback:** If the connection drops mid-flight, uncommitted spans are retained in a bounded client ring buffer up to a configurable threshold (e.g., 65,536 spans). If full, oldest spans are dropped to safeguard host memory.

### 5.2 Server Concurrency & Multiplexing
- The `IpcListener` accept loop runs on Tokio.
- Each accepted connection is spawned onto an isolated async task (`tokio::spawn`).
- A single daemon process can multiplex 64+ concurrent ASGI worker processes streaming simultaneously at full speed without thread contention.
- **Half-Closed Sockets & Sudden Terminations:** When a client worker process exits abruptly (`kill -9`), the server detects the `EOF` or broken pipe error cleanly, drops the connection task, and releases socket buffer allocations without logging noisy stack traces.

---

## 6. Defensive Guardrails & Edge Cases

1. **Stale Lock Cleanup:** Automated probe-and-remove ensures the daemon always recovers gracefully from system crashes without manual user intervention.
2. **Buffer Overrun Protection:** The 64 MB frame ceiling established in Phase 1 is enforced at the codec boundary. If a client attempts to transmit a frame exceeding 64 MB, the connection is immediately terminated to protect the daemon from memory exhaustion.
3. **Permission Hardening:** On Unix, socket directories and socket files are verified for secure permissions (`0700` directory, `0600` socket) to protect sensitive prompt strings and model responses from unauthorized local access.
4. **Cross-Platform Compilation:** Linux, macOS, and Windows compilation is verified with target-conditional compilation (`#[cfg(unix)]` and `#[cfg(windows)]`).

---

## 7. Verification, Integration & Benchmarking Plan

### 7.1 Integration Test Matrix
1. **End-to-End Client/Server Handshake:**
   - Client connects, sends `Handshake`, receives `HandshakeAck`, sends `Ping`, receives `Pong`.
2. **Concurrent Multi-Client Streaming:**
   - Spawn 16 concurrent client workers streaming 1,000 spans each over a single local UDS/pipe listener.
   - Verify that all 16,000 spans arrive intact with 0 lost frames.
3. **Stale Socket Self-Healing Test:**
   - Create a dummy file at `/tmp/poom-test.sock` without an active listener.
   - Boot the server on that path; verify the server detects the stale file, cleans it up, binds successfully, and accepts client connections.
4. **Sudden Disconnect & Reconnect Resilience:**
   - Connect client, stream data, kill server.
   - Verify client enters exponential backoff without panicking.
   - Restart server; verify client reconnects automatically and resumes transmission.

### 7.2 Throughput & Latency Benchmarks
- Benchmark sustained IPC throughput between two local processes:
  - **Target Throughput:** $> 100,000$ raw frames per second over Unix Domain Sockets on standard developer hardware.
  - **Target Latency:** Round-trip `Ping` $\rightarrow$ `Pong` completes in $< 50$ microseconds.
