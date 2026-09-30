# Poom: Pure Rust Embedded AI Observability Platform
## Master Architectural Blueprint & Implementation Roadmap

Poom is an ultra-low-overhead, local-first LLM/AI observability engine written in pure Rust. It replaces complex, resource-heavy multi-service stacks (e.g., Langfuse with PostgreSQL, ClickHouse, Redis, and Next.js) with:
1. An embedded, zero-copy, ACID B-Tree storage engine (**`redb`**).
2. A high-performance native desktop UI (**`Iced`** with `tiny-skia` software rasterization or `wgpu` hardware acceleration).
3. A cross-platform low-latency IPC socket daemon (Unix Domain Sockets / Windows Named Pipes).
4. A PyO3-powered FastAPI client providing non-blocking ASGI auto-instrumentation with sub-100μs latency overhead and zero GIL blocking.

---

## Workspace Crate Topology

To maintain strict modularity, high testability, and fast incremental builds, the project is structured as a multi-crate Cargo workspace:

```
poom/
├── Cargo.toml                     # Workspace root manifest
├── crates/
│   ├── poom-types/                # Universal data taxonomy & primitives (Trace, Span, Event, Evaluation)
│   ├── poom-protocol/             # Length-prefixed framing & postcard binary serialization
│   ├── poom-transport/            # Cross-platform IPC (UDS / Windows Named Pipes)
│   ├── poom-storage/              # redb ACID B-Tree tables, composite keys & query engine
│   ├── poom-daemon/               # Ingestion server, tokenizer, cost engine, micro-batcher
│   ├── poom-ui/                   # Iced desktop interface (Elm Architecture, Canvas Waterfall, Inspector)
│   └── poom-cli/                  # Unified CLI binary (dual-mode: GUI / headless daemon)
├── python/
│   ├── poom-py/                   # PyO3 C-extension bindings (Maturin-based)
│   └── poom/                      # Pure Python ergonomics (@trace, @trace_tool, TracerMiddleware)
├── docs/
│   └── plans/                     # Implementation phase specifications
│       ├── README.md              # Master roadmap (this file)
│       ├── phase-1-protocol-and-taxonomy.md
│       ├── phase-2-ipc-transport.md
│       ├── phase-3-storage-engine-redb.md
│       ├── phase-4-ingestion-daemon.md
│       ├── phase-5-python-instrumentation.md
│       ├── phase-6-iced-desktop-ui.md
│       └── phase-7-analytics-cli-packaging.md
```

---

## 7-Phase Modular Roadmap

To eliminate risk and avoid monolithic failure modes, the roadmap is decomposed into **7 tightly scoped phases**:

| Phase | Title | Primary Crates | Deliverables |
|---|---|---|---|
| **Phase 1** | **Universal Data Taxonomy & Wire Protocol** | `poom-types`, `poom-protocol`, `postcard`, `uuid` | Data models (Span, Trace, Event, Evaluation), UUIDv7, postcard binary codec, length framing format |
| **Phase 2** | **Cross-Platform IPC & Transport Engine** | `poom-transport`, `tokio`, `tokio-util` | Unix Domain Sockets (`/tmp/poom.sock`), Windows Named Pipes, length-delimited streaming, client/server connection loops |
| **Phase 3** | **Embedded Storage Engine on redb** | `poom-storage`, `redb`, `zerocopy` | Big-Endian composite keys, `SPANS`, `TIME_INDEX`, `TRACE_SPANS`, `SPAN_CHILDREN`, multi-reader isolation, micro-batch flush |
| **Phase 4** | **Ingestion Daemon & Metric Enrichment** | `poom-daemon`, `tiktoken-rs`, `tokenizers`, `flume` | Socket listener pool, token counting engine, dynamic cost lookup table, write-coalescing pipeline |
| **Phase 5** | **PyO3 Native Python Client & FastAPI Engine** | `poom-py`, `pyo3`, `crossbeam-channel`, `maturin` | GIL-free lock-free channel, background OS worker thread, `contextvars` hierarchy propagation, `@trace`, `@trace_tool`, ASGI middleware |
| **Phase 6** | **Iced Desktop Observability Studio** | `poom-ui`, `iced`, `iced_aw` | The Elm Architecture (TEA), live trace explorer table, 2D Gantt waterfall canvas (`canvas::Program`), span inspector |
| **Phase 7** | **Analytics, Prompt Diffing, CLI & Packaging** | `poom-cli`, `similar`, `plotters-iced` | Visual prompt diff viewer, latency percentiles & token charts, dual-mode CLI (`--headless` vs GUI), Maturin wheel distribution |

---

## System Performance & Target Invariants

1. **Client Latency Overhead:** Client-side span creation, context propagation, and IPC dispatch must complete in **< 100 microseconds (0.1ms)** per span.
2. **Ingestion Throughput:** The local daemon must sustain at least **50,000 spans/sec** without socket buffer overflow or UI frame stutter.
3. **Memory Footprint:** 
   - Cold startup daemon RSS: **< 15 MB**.
   - Desktop UI with 100,000 active spans: **< 50 MB** (via `tiny-skia` software backend).
4. **Reliability & Backpressure:** The Python client must never block the ASGI event loop or throw unhandled exceptions to user code if the daemon is offline.
