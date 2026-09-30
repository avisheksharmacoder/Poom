# Phase 3: Embedded Storage Engine & Index Topology on `redb`

## 1. Objective
Design and implement the embedded, zero-copy, ACID B-Tree storage engine using `redb`. Without relying on external SQL engines, `redb` will store millions of spans, support real-time chronological timeline queries, preserve parent-child span execution trees, and facilitate multi-attribute filtering.

---

## 2. Table Topology & Key Engineering

`redb` operates on pure byte-slice keys and values with typed definitions. Since B-Trees order keys lexicographically by byte order, composite key design is paramount.

```
redb Database File (e.g., ~/.poom/data.redb)
├── SPANS Table               [SpanId (16B) -> Postcard Span Blob]
├── TIME_INDEX Table          [(Timestamp_BE (8B) + TraceId (16B)) -> ()]
├── TRACE_SPANS Multimap      [TraceId (16B) -> SpanId (16B)]
├── SPAN_CHILDREN Multimap    [ParentSpanId (16B) -> ChildSpanId (16B)]
└── TAG_INDEX Table           [(TagKey + TagValue + TraceId) -> ()]
```

### 2.1 Table Specifications

1. **`SPANS` (`Table<&[u8; 16], &[u8]>`):**
   - **Key:** `SpanId` (16 bytes UUIDv7).
   - **Value:** Postcard-serialized `SpanRecord` bytes.
   - O(1) point-lookup for any individual span.

2. **`TIME_INDEX` (`Table<&[u8; 24], ()>`):**
   - **Key:** `Timestamp_BE` (8 bytes, Big-Endian `u64` nanoseconds) + `TraceId` (16 bytes).
   - **Value:** `()` (zero-sized unit type).
   - **Rationale:** Storing timestamps in Big-Endian (`u64::to_be_bytes()`) ensures natural chronological byte ordering in the B-Tree.
   - **Query:** Fast reverse range scanning (`.iter().rev()`) yields the most recent traces in microseconds with zero allocations.

3. **`TRACE_SPANS` (`MultimapTable<&[u8; 16], &[u8; 16]>`):**
   - **Key:** `TraceId` (16 bytes).
   - **Value:** `SpanId` (16 bytes).
   - Instant extraction of all span IDs belonging to a root trace in a single key seek.

4. **`SPAN_CHILDREN` (`MultimapTable<&[u8; 16], &[u8; 16]>`):**
   - **Key:** `ParentSpanId` (16 bytes).
   - **Value:** `ChildSpanId` (16 bytes).
   - Enables recursive or iterative reconstruction of nested agent execution trees and sub-tool invocations.

5. **`TAG_INDEX` (`Table<&[u8], ()>`):**
   - **Key:** Composite byte slice `[tag_key_len: u16][tag_key][tag_val_len: u16][tag_val][trace_id: 16B]`.
   - **Value:** `()`.
   - Supports prefix scanning for filtering traces by status, environment, model name, or user tags.

---

## 3. Concurrency & Micro-Batching

- **Single-Writer / Multi-Reader Model:**
  - `redb` enforces single-writer exclusivity via file locks while permitting concurrent, non-blocking read transactions.
  - The ingestion worker holds write operations on a dedicated writer task.
  - The Iced desktop UI creates lightweight, read-only snapshot transactions without blocking active ingestion.
- **Write Micro-Batching:**
  - Writing every span individually causes excessive disk syncs.
  - Spans are queued in memory and committed in a single ACID transaction under dual flush triggers:
    1. **Size trigger:** Once **1,000 spans** accumulate.
    2. **Timeout trigger:** Every **50 milliseconds** (ensuring live UI reactivity).
- **Retention & Data Pruning (TTL):**
  - Background maintenance task scans `TIME_INDEX` from the start of the B-Tree up to a cutoff timestamp (e.g. 7 days or 30 days).
  - Deletes expired records across all tables in a single compaction transaction.

---

## 4. Deliverables in `crates/poom-storage`

- `db.rs`: `StorageEngine` handle wrapping `redb::Database`.
- `schema.rs`: Table definitions, key serializers, and Big-Endian encoding utilities (`zerocopy`).
- `writer.rs`: `BatchWriter` worker managing write transactions and dual-trigger flushing.
- `reader.rs`: `ReadOnlyStorage` interface for querying traces, spans, and tree structures.
- `pruner.rs`: Time-To-Live (TTL) retention pruning task.
- `tests/storage_bench.rs`: Benchmark commits under high-concurrency read/write load.

---

## 5. Verification & Acceptance Criteria
- [ ] Atomically commit 10,000 spans in under 50ms in micro-batched transactions.
- [ ] Sub-millisecond reverse chronological page scan of top 100 recent traces.
- [ ] Simultaneous concurrent reads during heavy write ingestion without deadlocks or latency degradation.
- [ ] Clean recovery and integrity validation upon simulated abrupt process termination.
