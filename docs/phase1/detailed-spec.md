# Phase 1: Universal Data Taxonomy & Wire Protocol
## Detailed Architectural & Mechanical Specification

---

## 1. Scope, Purpose & Architectural Boundary

Phase 1 provides the foundational data modeling and binary framing contracts for the entire Poom ecosystem. 

To ensure long-term stability and prevent cascading dependency bugs, Phase 1 is strictly isolated:
- **Zero I/O, Async, or OS Dependencies:** It contains no socket code, file system access, or async runtimes. It is pure, deterministic Rust that compiles in milliseconds.
- **Single Source of Truth:** All subsequent crates (`poom-transport`, `poom-storage`, `poom-daemon`, `poom-py`, `poom-ui`, and `poom-cli`) depend on this layer to speak an identical memory and wire language.
- **Zero Schema Compilation:** Unlike Protobuf or gRPC, which require external compilers (`protoc`) and code generation steps, Poom uses a native Rust binary codec (`postcard`) coupled to Serde, maintaining maximum performance with zero external build toolchain friction.

---

## 2. Identity Engineering: Time-Ordered UUIDv7

### How Identifiers Work in Poom
Every execution boundary (`Trace`) and execution block (`Span`) requires an identifier that is simultaneously globally unique, collision-free across concurrent worker processes, and naturally ordered by time.

Traditional UUIDv4 identifiers are completely random. When stored in a B-Tree storage engine like `redb`, random keys cause random page splits, high disk fragmentation, and require a secondary time index table just to query recent traces.

Poom solves this by standardizing on **UUIDv7**:
1. **Bit Layout & Time Encoding:**
   - The first 48 bits encode a Unix millisecond timestamp.
   - The remaining 74 bits contain cryptographically strong pseudo-random data with sub-millisecond sequencing.
2. **Natural B-Tree Ordering:**
   - Because the high-order bits are chronological, lexicographical byte comparison of two UUIDv7 values matches their creation timeline.
   - Querying recent spans or traces translates directly to a backward iterator range scan (`.iter().rev()`) across the primary B-Tree.
3. **Embedded Timestamp Extraction:**
   - The exact millisecond timestamp can be unpacked directly from the UUIDv7 bytes without storing a redundant `created_at` timestamp field in index tables.
4. **Type-Safe Newtype Wrappers:**
   - `TraceId` and `SpanId` are wrapped as distinct transparent newtypes (`#[repr(transparent)]`).
   - This ensures a 16-byte memory footprint identical to `[u8; 16]` while making it impossible at compile time to accidentally pass a `SpanId` into an API expecting a `TraceId`.

---

## 3. Memory Allocation Strategy & In-Memory Layout

A core design mandate of Poom is minimal memory consumption (< 15 MB RSS) and sub-microsecond tracing overhead. High-throughput tracing systems often suffer from **heap allocator churn** due to repeatedly allocating tiny strings (e.g., span names like `"llm_call"`, attribute keys like `"model"`, error types like `"ValueError"`).

### 3.1 Stack Allocation for Small Strings via `SmolStr`
- Standard `String` in Rust requires 24 bytes on the stack plus a separate heap allocation, even for a 4-letter string.
- Poom uses `SmolStr` for names, error classes, model identifiers, and attribute keys.
- Strings up to 23 bytes are stored **inline on the stack** without hitting the heap allocator. Heap allocation is only triggered when strings exceed 23 bytes.

### 3.2 Deterministic Canonical Serialization via `BTreeMap`
- For dynamic span attributes and metadata, Poom chooses `BTreeMap` over `HashMap`.
- `HashMap` iteration order is randomized by design (SipHash randomized keys to prevent DoS attacks). This makes binary serialization non-deterministic—the same span data serialized twice produces different byte sequences.
- `BTreeMap` maintains keys in strict sorted order. This produces **canonical binary serialization**, which is essential for:
  - Byte-level deduplication and content hashing.
  - Reproducible caching and instant equality checks.
  - Consistent prompt and attribute diffing.

---

## 4. Universal Data Taxonomy Mechanics

The taxonomy models the complete lifecycle of AI workflows—from multi-agent loops to individual model token consumption.

### 4.1 Hierarchical Spans & Kinds
Each timed operation is categorized by a single-byte wire discriminant:
- **`Agent`:** High-level orchestrator, state loop, or multi-step reasoning agent.
- **`Chain`:** Sequential pipeline, DAG step, or retrieval workflow (e.g. LangChain / LlamaIndex pipelines).
- **`Llm`:** Direct model inference call (recording prompts, completions, temperature, model identifier, tokens).
- **`Tool`:** External tool or function invocation (recording input parameters, return payloads, schema validation).
- **`Function`:** Internal application code blocks, utility functions, or database queries.
- **`Http`:** Incoming HTTP/ASGI boundary requests (recording routes, status codes, query parameters).

### 4.2 Hierarchy Resolution
- Every span carries a mandatory `trace_id` and `span_id`.
- Root spans have `parent_span_id: None`.
- Child operations carry `parent_span_id: Some(parent_id)`.
- When an agent calls a chain, which in turn calls an LLM and a tool, the parent-child pointers preserve the full directed tree across concurrent async tasks.

### 4.3 Execution Status & Rich Errors
- Operations complete as either `Ok` or `Error`.
- Error statuses capture the error classification (inline `SmolStr`), human-readable error message, and an optional structured stacktrace (capturing Python or Rust tracebacks).

### 4.4 Flexible Attribute Value System
To accommodate arbitrary metadata from user applications, attributes support a dynamic value enum:
- Primitive scalars: Booleans, 64-bit signed integers, 64-bit floats.
- Text: Strings and inline small strings.
- Binary blobs: Raw bytes for embeddings or binary payloads.
- Nested structures: Arrays of values and maps of string-to-value.
- Seamless conversion to and from JSON (`serde_json::Value`).

### 4.5 Granular Metric & Cost Accounting
LLM spans record metrics with dedicated fields:
- `input_tokens`: Prompt tokens sent to the model.
- `output_tokens`: Completion tokens generated.
- `total_tokens`: Sum of input and output tokens.
- `cached_tokens`: Context-cached tokens read at lower cost.
- `reasoning_tokens`: Internal reasoning tokens (e.g. OpenAI o1/o3 architectures).
- `estimated_cost_usd`: Pre-computed or dynamically enriched financial cost in US Dollars.
- Tokens are modeled as optional 32-bit unsigned integers, optimizing wire footprint under variable-length integer encoding.

### 4.6 Events & Evaluations
- **`EventRecord` (Point-in-Time Log):** Represents instantaneous occurrences attached to a span that do not have a duration (e.g., prompt guardrail triggers, cache hits, streaming chunk milestones).
- **`EvaluationRecord` (Assessment / Score):** Quantitative or qualitative evaluations linked to a trace or span. Can represent automated evaluations (e.g. hallucination score, toxicity score) or human feedback (thumbs-up/down, user ratings, comments).

---

## 5. Binary Wire Protocol & Framing Mechanics

When the instrumented application sends spans to the local daemon over IPC, data must be framed cleanly to handle stream fragmentation and prevent buffer exploits.

### 5.1 The 10-Byte Length-Prefixed Wire Frame Layout

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|       Magic (0x50, 0x4D)      |      Protocol Version         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|             Flags             |        Payload Length         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    Payload Length (cont.)     |     Payload Bytes (N bytes)   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                            ...                                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

#### Field Specifications:
1. **Magic Bytes (2 Bytes - `[0x50, 0x4D]`):**
   - ASCII for `'P'`, `'M'` ("Poom").
   - If incoming stream bytes do not start with these magic bytes, the decoder immediately flags stream corruption, drops the socket connection, and prevents processing arbitrary garbage bytes.
2. **Protocol Version (2 Bytes - `u16` Little-Endian):**
   - Initial version is `1`.
   - Allows future protocol iterations to coexist or gracefully negotiate capabilities.
3. **Flags (2 Bytes - `u16` Little-Endian):**
   - Bit 0: Compression enabled (e.g. LZ4 block compression for large prompt traces).
   - Bit 1: Encryption enabled (reserved).
   - Bits 2-15: Reserved for future extensions.
4. **Payload Length (4 Bytes - `u32` Little-Endian):**
   - Specifies the exact byte count $N$ of the serialized postcard payload immediately following the header.
   - Guardrail ceiling: Enforces a hard maximum frame size of **64 MB**. Any header advertising a payload larger than 64 MB is rejected before memory is allocated.

---

## 6. How the Streaming Buffer & Framing State Machine Works

In local IPC streaming (Unix Domain Sockets or Named Pipes), the OS delivers data in arbitrary byte chunks. A 5,000-byte frame might arrive in three separate socket reads (e.g., 1024 bytes, 2048 bytes, 1928 bytes), or a single socket read might deliver ten batched frames at once.

### 6.1 The Sliding Window Reassembly Loop
The frame decoder operates on a mutable byte buffer (`bytes::BytesMut`):
1. **Header Check:** The decoder checks if the buffer contains at least 10 bytes. If fewer than 10 bytes are present, it yields and waits for more socket data.
2. **Magic Byte Validation:** It inspects the first 2 bytes. If they do not match `[0x50, 0x4D]`, an unrecoverable protocol error is raised.
3. **Length Extraction:** It reads the payload length $N$ without consuming the buffer.
4. **Capacity Reservation:** If the total frame size ($10 + N$) exceeds the current buffer length, the decoder calculates the deficit and reserves capacity in `BytesMut`, avoiding repeated incremental reallocations.
5. **Zero-Copy Payload Extraction:** Once the buffer contains at least $10 + N$ bytes:
   - The 10-byte header is advanced and discarded.
   - The exact $N$ payload bytes are split off zero-copy via `BytesMut::split_to(N)`.
   - The remaining bytes in the buffer stay in place for the next frame iteration.
6. **Deserialization:** The split byte slice is handed directly to `postcard::from_bytes`, reconstructing strongly typed internal message structs.

---

## 7. Message Types & Protocol Flow

Communication between the client (Python application) and the daemon consists of structured messages:

### 7.1 Client Messages
- **`Handshake`:** Sent immediately upon opening the IPC connection. Informs the daemon of the client library version, process ID (PID), and host application name.
- **`IngestSpans`:** The primary high-throughput ingestion message carrying a vector of batched spans (`Vec<SpanRecord>`).
- **`IngestEvaluations`:** Carries a vector of batched evaluations or scores (`Vec<EvaluationRecord>`).
- **`Ping`:** Periodic heartbeat carrying a nanosecond timestamp to verify IPC connection liveness and measure local round-trip latency.
- **`Disconnect`:** Clean termination notification sent before the instrumented process exits.

### 7.2 Server Messages
- **`HandshakeAck`:** Daemon confirms connection acceptance, returns server version, and declares operational constraints (such as maximum batch size).
- **`IngestAck`:** Acknowledges successful receipt and processing of a batch, returning the count of accepted records.
- **`Pong`:** Response to client ping carrying the original timestamp.
- **`Error`:** Error notification detailing rejected frames, unsupported versions, or backpressure rejections.

---

## 8. Invariants & Defensive Guardrails

To prevent crashes, resource starvation, or silent data corruption:
1. **Timestamp Monotonicity:** A span's `end_time_unix_nanos` must be greater than or equal to its `start_time_unix_nanos`.
2. **Root Span Guarantee:** Every trace must possess exactly one root span (`parent_span_id == None`). All other spans within the trace must point to a valid parent.
3. **Non-Empty Entity Names:** Spans, events, and metrics cannot have empty string names.
4. **Memory Guardrail:** The frame decoder strictly enforces the 64 MB maximum frame size, preventing malicious or corrupted socket streams from triggering Out-Of-Memory (OOM) conditions.
5. **Deterministic Serialization:** All maps are ordered by key, ensuring that identical semantic payloads produce identical byte slices.

---

## 9. Verification, Fuzzing & Microbenchmarking Strategy

Phase 1 validation is split into three rigorous verification tiers:

### 9.1 Property-Based Fuzzing (`proptest`)
- Uses property testing to generate thousands of randomized, deeply nested span trees containing:
  - Unicode strings, emoji, and zero-width characters in names and attributes.
  - Floating point boundary values (NaN, subnormals, negative zero, infinity).
  - Deep parent-child nesting hierarchies (up to 64 levels deep).
- Invariant: `decode(encode(SpanRecord)) == SpanRecord` must hold true across 100% of generated cases.

### 9.2 Stream Fragmentation Fuzzing
- Simulates worst-case socket delivery behavior:
  - **1-Byte Trickle Stream:** Feeds a valid 10,000-byte frame into the decoder 1 byte at a time to verify sliding window reassembly.
  - **Multi-Frame Bursts:** Concatenates 50 distinct frames into a single contiguous buffer and verifies the decoder yields all 50 frames without dropping bytes.
  - **Corrupted Stream Injection:** Flips random bits in magic bytes, versions, and payload lengths to verify that the decoder rejects corrupted data gracefully without panicking.

### 9.3 Performance Microbenchmarking (`criterion`)
- Measures in-memory encoding and decoding speed under release compilation:
  - **Target 1:** Encoding a standard LLM span payload takes `< 1.5 microseconds`.
  - **Target 2:** Decoding a standard LLM span payload takes `< 2.0 microseconds`.
  - **Target 3:** Header parsing and validation overhead is `< 50 nanoseconds` with **0 heap allocations**.
  - **Target 4:** In-memory batch serialization sustains `> 500,000 spans/second`.
