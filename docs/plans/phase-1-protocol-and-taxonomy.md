# Phase 1: Universal Data Taxonomy & Wire Protocol

## 1. Objective
Establish the foundational data models, primitives, serialization format, and length-delimited byte framing protocol for Poom. This phase has zero network or runtime dependencies and serves as the single source of truth for all downstream crates.

---

## 2. Core Concepts & Taxonomy

### 2.1 Identifiers & Ordering
- **UUIDv7:** Used for all `TraceId` and `SpanId` identifiers.
  - Generates time-sortable 128-bit identifiers based on Unix millisecond timestamps with sub-millisecond randomness.
  - Eliminates secondary sorting indices in B-tree stores (`redb`).
  - Implemented via `uuid = { version = "1", features = ["v7", "serde"] }`.

### 2.2 Data Taxonomy Primitives

1. **`SpanKind` Enum:**
   - `Agent`: High-level orchestration, state loop, or multi-step reasoning.
   - `Chain`: Sequential pipeline, DAG step, or retrieval workflow.
   - `Llm`: Direct model inference call (prompts, completions, temperature, model identifier, tokens).
   - `Tool`: External function invocation (arguments, return outputs, schema validation).
   - `Function`: Arbitrary internal code block or utility function.

2. **`SpanStatus` Enum:**
   - `Ok`: Successful execution.
   - `Error`: Execution failure with error type, message, and backtrace.

3. **`SpanRecord` Struct:**
   - `trace_id`: `TraceId` (UUIDv7)
   - `span_id`: `SpanId` (UUIDv7)
   - `parent_span_id`: `Option<SpanId>`
   - `name`: `String` (or compact `CompactString` / `SmolStr` to reduce allocations)
   - `kind`: `SpanKind`
   - `start_time_unix_nanos`: `u64`
   - `end_time_unix_nanos`: `Option<u64>`
   - `status`: `SpanStatus`
   - `attributes`: `HashMap<String, AttributeValue>` (or BTreeMap for deterministic serialization)
   - `events`: `Vec<EventRecord>`
   - `metrics`: `SpanMetrics` (tokens, cost, duration)

4. **`EventRecord` Struct (Point-in-Time Snapshot):**
   - `timestamp_unix_nanos`: `u64`
   - `name`: `String`
   - `attributes`: `HashMap<String, AttributeValue>`

5. **`EvaluationRecord` Struct (Scores & Feedback):**
   - `trace_id`: `TraceId`
   - `span_id`: `Option<SpanId>`
   - `name`: `String` (e.g. "hallucination", "user_feedback")
   - `value`: `EvaluationValue` (Float, Boolean, Categorical)
   - `comment`: `Option<String>`

---

## 3. Wire Protocol & Serialization

### 3.1 Serialization Format
- Serialization uses **`postcard`**:
  - `no_std` compatible, zero-overhead, highly compact binary format.
  - Varint integer encoding minimizes byte footprint.
  - Avoids protobuf/schema compiler compilation steps while providing strict type safety.

### 3.2 Wire Frame Structure
Every frame transmitted across the IPC socket consists of:
```
+-------------------+----------------------+---------------------------------+
| Magic Bytes (2B)  | Payload Length (4B)  | Postcard Serialized Payload (NB)|
| 0x50 0x4D ("PM")  | Little-Endian u32    | Length N bytes                  |
+-------------------+----------------------+---------------------------------+
```
- **Magic Bytes:** `0x50, 0x4D` (`PM` for Poom) to detect stream desynchronization and corrupted frames.
- **Protocol Version:** Embedded in the magic or as a header (`u16`).
- **Length Prefix:** 4 bytes unsigned 32-bit integer in little-endian format. Limits maximum frame size to 64 MB (configurable).

---

## 4. Deliverables in Crates

### `crates/poom-types`
- `types.rs`: `TraceId`, `SpanId`, `SpanKind`, `SpanStatus`, `AttributeValue`, `SpanRecord`, `EventRecord`, `EvaluationRecord`.
- `tests/taxonomy_tests.rs`: Serialization and roundtrip tests.

### `crates/poom-protocol`
- `frame.rs`: `FrameEncoder`, `FrameDecoder`, magic byte validation, length validation.
- `payload.rs`: `ClientMessage` enum (`IngestSpans(Vec<SpanRecord>)`, `IngestEvaluations(Vec<EvaluationRecord>)`, `Heartbeat`).
- `tests/protocol_roundtrip.rs`: Comprehensive round-trip fuzz testing and property tests via `proptest`.

---

## 5. Verification & Acceptance Criteria
- [ ] 100% round-trip fidelity: `SpanRecord -> Postcard -> Bytes -> Postcard -> SpanRecord`.
- [ ] Frame validation rejects invalid magic bytes or oversized frames without memory leaks.
- [ ] Zero heap allocations during frame header decoding.
- [ ] Micro-benchmark: Encoding a standard LLM span payload takes `< 2 microseconds`.
