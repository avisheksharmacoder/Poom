# Phase 4: Standalone Ingestion Daemon & Metrics Enrichment

## 1. Objective
Build the core backend daemon responsible for hosting the IPC server, demuxing incoming streams, enriching LLM spans with token counts and dynamic financial cost calculations, and feeding the micro-batch storage writer.

---

## 2. Architecture & Runtime Isolation

### 2.1 Tuned Tokio Runtime
- Constrain Tokio to 2 worker threads (`tokio::runtime::Builder::new_multi_thread().worker_threads(2)`).
- Ensures the daemon stays idle at < 0.1% CPU when no requests are flowing.
- Caps OS thread stack bloat and maintains RSS below 15 MB.

### 2.2 Enrichment Pipeline
```
[IPC Stream (Framed Postcard)]
             │
             ▼
[Frame Demuxer & Validator]
             │
             ▼
[Enrichment Worker Pool]
  ├─ Check if Span is SpanKind::Llm
  ├─ If tokens missing: Lazy Tokenizer (tiktoken-rs / tokenizers)
  └─ Calculate Cost (Dynamic Model Pricing Table)
             │
             ▼
[Flume MPMC Channel]
             │
             ▼
[Micro-Batch Storage Writer] ──> [redb ACID Commit]
             │
             ▼
[Broadcast Channel] ──> [Live UI Subscription / Event Stream]
```

### 2.3 Lazy Tokenizer Engine
- Eagerly loading BPE rank tables (like `cl100k_base`, `o200k_base`) on process boot wastes 30+ MB of memory.
- Use `OnceCell` or `LazyLock` to defer tokenizer initialization until an LLM span lacking explicit token metrics arrives.
- Supported tokenizers: `tiktoken-rs` (OpenAI cl100k, o200k, p50k) and HuggingFace `tokenizers` for open-source / Anthropic models.

### 2.4 Dynamic Model Cost Engine
- In-memory pricing lookup table mapping `(model_name, date)` to input, output, and cached token pricing per 1M tokens.
- Serde-deserializable configuration file (`~/.poom/pricing.toml`) allowing users to override or define custom fine-tuned model costs without recompilation.

---

## 3. Deliverables in `crates/poom-daemon`

- `server.rs`: Daemon orchestrator accepting incoming connections on `poom-transport`.
- `pipeline.rs`: Ingestion stage receiving frames and routing to enrichment workers.
- `tokenizer.rs`: Lazy tokenizer wrapper (`tiktoken-rs` / `tokenizers`).
- `cost.rs`: Model pricing table, fallback heuristics, and cost calculations.
- `broadcast.rs`: Tokio broadcast channel dispatching live span events to connected UI subscribers.
- `tests/daemon_pipeline_tests.rs`: End-to-end ingestion pipeline simulation with simulated LLM payloads.

---

## 4. Verification & Acceptance Criteria
- [ ] Process memory remains < 15 MB RSS before the first LLM tokenization is triggered.
- [ ] Correctly calculates token counts and financial costs for major model signatures (GPT-4o, Claude 3.5 Sonnet, Llama 3, Gemini 1.5).
- [ ] Handles 50,000 incoming spans/sec over local IPC without message loss or queue overflow.
- [ ] Broadcast channel provides real-time event notifications to listeners without backpressure choking the ingestion pipeline.
