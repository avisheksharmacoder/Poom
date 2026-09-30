# Phase 4: Standalone Ingestion Daemon & Metrics Enrichment
## Detailed Architectural & Mechanical Specification

---

## 1. Scope, Purpose & Architectural Boundary

Phase 4 constructs **`poom-daemon`**, the central background engine that coordinates socket listeners, client sessions, metadata enrichment, dynamic model cost computation, and persistence dispatch.

### 1.1 Core Responsibilities
1. **Asynchronous Socket Multiplexing:** Hosts the `IpcListener` created in Phase 2, continuously accepting connections from multiple ASGI workers (FastAPI, Granian, Gunicorn, Uvicorn) without thread blocking.
2. **Minimal Runtime Footprint:** Capped Tokio runtime workers ensuring the daemon idles at $< 0.1\%$ CPU and $< 15$ MB RSS.
3. **Lazy Tokenization Engine:** On-demand BPE token counting that defers loading heavy vocabularies until an uncounted LLM span actually arrives.
4. **Dynamic Model Pricing & Cost Accounting:** Automatic calculation of financial cost (USD) for major LLM providers (OpenAI, Anthropic, Gemini, Mistral, Meta Llama, DeepSeek) based on prompt, completion, and cached tokens.
5. **Real-Time UI Broadcast Engine:** A lock-free broadcast channel that delivers live span notifications to connected Iced desktop UI subscribers with zero persistence backpressure.

---

## 2. Ingestion Pipeline & Concurrency Architecture

The daemon operates as an asynchronous, multi-stage processing pipeline:

```
[FastAPI / Client Process 1] ──┐
[FastAPI / Client Process 2] ──┼──> [IpcListener (/tmp/poom.sock)]
[FastAPI / Client Process N] ──┘                    │
                                                    ▼
                                    ┌───────────────────────────────┐
                                    │ Connection Session Handler    │
                                    │ (tokio::spawn per connection) │
                                    └───────────────┬───────────────┘
                                                    │
                                     Incoming ClientMessage::IngestSpans
                                                    │
                                                    ▼
                                    ┌───────────────────────────────┐
                                    │   Span Enrichment Stage       │
                                    │   - Check SpanKind == Llm     │
                                    │   - Lazy Token Counting       │
                                    │   - Dynamic Pricing Lookup    │
                                    └───────────────┬───────────────┘
                                                    │
                               Enriched Spans (Tokens & USD Cost computed)
                                                    │
                         ┌──────────────────────────┴──────────────────────────┐
                         ▼                                                     ▼
        ┌───────────────────────────────────┐               ┌────────────────────────────────────┐
        │ BatchWriter Ingestion Queue       │               │ Live Event Broadcast Channel       │
        │ (poom-storage redb write loop)    │               │ (tokio::sync::broadcast)           │
        └─────────────────┬─────────────────┘               └─────────────────┬──────────────────┘
                          │                                                   │
                          ▼                                                   ▼
                [~/.poom/data.redb]                                   [Iced Desktop UI]
```

### 2.1 Connection Lifecycle & Protocol Flow
1. **Handshake Verification:**
   - When a client connects, the session handler expects `ClientMessage::Handshake`.
   - The daemon verifies protocol compatibility, records the client PID and application name, and responds with `ServerMessage::HandshakeAck`.
2. **Streaming Batch Ingestion:**
   - Client sends `ClientMessage::IngestSpans(Vec<SpanRecord>)`.
   - Spans are passed directly to the Enrichment Stage.
   - Enriched spans are pushed into the storage `BatchWriter` queue.
   - The daemon sends `ServerMessage::IngestAck` confirming receipt and batch acceptance.
3. **Latency Heartbeat (Ping-Pong):**
   - When receiving `ClientMessage::Ping { timestamp_nanos }`, the session immediately echoes back `ServerMessage::Pong { timestamp_nanos }` in under 10 microseconds.
4. **Clean Disconnect & EOF Trapping:**
   - Upon receiving `ClientMessage::Disconnect` or detecting socket `EOF`, the session task exits cleanly, decrementing the active connection counter.

---

## 3. Lazy Tokenizer Architecture

A major source of memory bloat in AI monitoring tools is eager tokenizer rank loading. Dictionaries like OpenAI's `cl100k_base` and `o200k_base` allocate 15 MB to 30 MB of heap memory as soon as they are loaded into RAM.

### 3.1 The On-Demand Initialization Mechanism
- The tokenizer engine wraps BPE tokenizers behind a thread-safe `OnceLock` or `LazyLock`.
- When the daemon starts, **zero tokenizer vocabularies are loaded into memory**, preserving the sub-15 MB RSS profile.
- Tokenization is only triggered when **both** conditions are met:
  1. The span is `SpanKind::Llm`.
  2. The span metrics lack explicit token counts (`metrics.input_tokens.is_none()` or `metrics.output_tokens.is_none()`).
- If user code already provides token counts (reported directly by the model API), tokenization is bypassed entirely, achieving zero additional CPU overhead.

### 3.2 Supported Model Vocabularies
- **`cl100k_base`:** Used by GPT-4, GPT-4-Turbo, GPT-3.5-Turbo, text-embedding-ada-002.
- **`o200k_base`:** Used by GPT-4o, GPT-4o-mini, OpenAI o1, o3 series.
- **`p50k_base` / `r50k_base`:** Legacy models (text-davinci-003, Codex).
- **Fast Fallback Estimator:** For open-source or unrecognized models (e.g. Anthropic, Gemini, Mistral, Llama), uses a high-throughput subword character-ratio estimator (~3.8 characters per token for English text) to provide approximate counts without allocating multi-megabyte vocabularies.

---

## 4. Dynamic Model Pricing & Financial Cost Engine

To understand LLM expenditure, every inference span must carry an accurate estimated financial cost.

### 4.1 Normalized Model Matching
LLM API responses often return versioned model strings (e.g. `"gpt-4o-2024-08-06"`, `"claude-3-5-sonnet-20241022"`, `"gemini-1.5-pro-002"`).
- The pricing engine normalizes model names using a canonical alias trie / prefix scanner:
  - `"gpt-4o-2024-08-06"` $\rightarrow$ matches `"gpt-4o"`.
  - `"claude-3-5-sonnet-latest"` $\rightarrow$ matches `"claude-3-5-sonnet"`.
  - Case-insensitive, stripping provider prefixes (e.g., `"openai/gpt-4o"` $\rightarrow$ `"gpt-4o"`).

### 4.2 Standard Model Pricing Catalog
Built-in baseline pricing table (rates per 1,000,000 tokens in USD):

| Model Family | Canonical Model ID | Prompt Cost / 1M | Completion Cost / 1M | Cached Prompt Cost / 1M |
|---|---|---|---|---|
| **OpenAI** | `gpt-4o` | $2.50 | $10.00 | $1.25 |
| **OpenAI** | `gpt-4o-mini` | $0.15 | $0.60 | $0.075 |
| **OpenAI** | `o1` | $15.00 | $60.00 | $7.50 |
| **OpenAI** | `o3-mini` | $1.10 | $4.40 | $0.55 |
| **Anthropic** | `claude-3-5-sonnet` | $3.00 | $15.00 | $0.30 |
| **Anthropic** | `claude-3-5-haiku` | $0.80 | $4.00 | $0.08 |
| **Anthropic** | `claude-3-opus` | $15.00 | $75.00 | $1.50 |
| **Google** | `gemini-1.5-pro` | $1.25 | $5.00 | $0.3125 |
| **Google** | `gemini-1.5-flash` | $0.075 | $0.30 | $0.01875 |
| **DeepSeek** | `deepseek-chat` | $0.14 | $0.28 | $0.014 |
| **DeepSeek** | `deepseek-r1` | $0.55 | $2.19 | $0.14 |
| **Meta Llama** | `llama-3.1-70b` | $0.59 | $0.79 | $0.30 |
| **Meta Llama** | `llama-3.1-8b` | $0.05 | $0.08 | $0.02 |

### 4.3 Cost Formula
$$\text{Cost} = \frac{\text{cached\_tokens} \times \text{cached\_rate} + (\text{input\_tokens} - \text{cached\_tokens}) \times \text{prompt\_rate} + \text{output\_tokens} \times \text{completion\_rate}}{1,000,000}$$

### 4.4 User Custom Overrides (`pricing.toml`)
Users can configure custom or self-hosted pricing by placing a `~/.poom/pricing.toml` file. The daemon loads custom rates at boot, overriding or augmenting the internal catalog without requiring Rust recompilation.

---

## 5. Live UI Broadcast Channel (`tokio::sync::broadcast`)

The desktop studio UI requires real-time updates as spans arrive, without polling the database in tight loops.

### 5.1 Architecture & Backpressure Isolation
- An in-memory broadcast channel (`tokio::sync::broadcast`) with a bounded ring buffer (e.g. 4,096 events).
- As enriched spans are written to the database batch writer, a copy or reference is emitted over the broadcast channel.
- **Zero Ingestion Blocking:** If a UI subscriber is sluggish or temporarily paused during a window resize, the broadcast channel drops older events for that specific subscriber (`RecvError::Lagged`) rather than stalling the ingestion server or delaying FastAPI responses.
- Desktop UI instances subscribe to this channel via `subscribe()`, displaying live execution waterfalls as requests happen.

---

## 6. Server State, Metrics & Telemetry

The daemon maintains an atomic operational state:
- `start_time`: Instant daemon booted.
- `active_connections`: Atomic gauge of concurrent connected client sockets.
- `total_spans_ingested`: Atomic counter of total received spans.
- `total_evaluations_ingested`: Atomic counter of received evaluations.
- `total_bytes_received`: Atomic counter of raw bytes ingested over IPC.
- `total_estimated_cost_usd`: Running accumulator of financial cost.

---

## 7. Graceful Shutdown & Signal Trapping

To avoid corrupting databases or dropping in-flight spans:
1. The daemon listens for `SIGINT` (Ctrl+C) and `SIGTERM` signals using `tokio::signal`.
2. When triggered:
   - Sets a shutdown cancellation token (`CancellationToken`).
   - Stops accepting new IPC connections on `IpcListener`.
   - Sends clean termination signals to active client sessions.
   - Triggers `BatchWriter::flush()` to force all in-memory micro-batches to disk in `redb`.
   - Closes the storage engine cleanly.
   - Exits process with code `0`.

---

## 8. Verification, Integration & Benchmarking Plan

### 8.1 Integration Test Matrix
1. **End-to-End Ingestion Flow:**
   - Spin up `PoomDaemon` on a test socket and test `redb` instance.
   - Connect client via `poom-transport`.
   - Send `Handshake`, send batch of 100 spans with model names, send `Disconnect`.
   - Query `StorageReader`: verify all 100 spans exist in `redb` with enriched token counts and calculated USD costs.
2. **Dynamic Cost Engine Accuracy:**
   - Verify cost calculation for GPT-4o: 1,000 input tokens, 500 output tokens, 200 cached tokens. Assert cost matches formula exactly down to 6 decimal places.
3. **Lazy Tokenizer Verification:**
   - Boot daemon. Inspect memory footprint ($< 15$ MB).
   - Ingest an LLM span without token metrics. Verify token count is automatically computed from prompt text.
4. **Live UI Broadcast Test:**
   - Subscribe to daemon broadcast channel.
   - Ingest 50 spans over client socket.
   - Verify subscriber receives all 50 live span events.
5. **High-Concurrency Stress Test:**
   - 16 concurrent clients streaming 10,000 spans simultaneously through the daemon into storage.
   - Verify 0 dropped spans, 0 memory leaks, and sub-100μs processing overhead.

### 8.2 Performance Targets
- **Enrichment Latency:** Less than **2.0 microseconds** per span (cost lookup + metric normalization).
- **Daemon Ingestion Throughput:** Sustains **> 50,000 spans/sec** across local IPC into `redb`.
- **Idle Memory Ceiling:** Under **15 MB RSS** before first tokenization.
