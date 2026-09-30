# Phase 3: Embedded Storage Engine & Index Topology on `redb`
## Detailed Architectural & Mechanical Specification

---

## 1. Scope, Purpose & Architectural Boundary

Phase 3 implements the embedded, zero-copy, ACID B-Tree persistence engine using **`redb`** in `crates/poom-storage`.

### 1.1 Why `redb` Over SQL, ClickHouse, or SQLite?
Traditional observability engines (such as Langfuse or Phoenix) require either external relational databases (PostgreSQL), distributed columnar stores (ClickHouse), or SQLite with external C-library linkages.
1. **Zero External Daemon Dependencies:** `redb` is a pure Rust, embedded key-value and multimap store. It compiles directly into the Poom binary with zero C-dependencies (`no_std` compatible storage layers).
2. **Zero-Copy & Native MVCC:** Multi-Version Concurrency Control (MVCC) allows concurrent readers to query snapshots without blocking the active write transaction.
3. **Pure B-Tree Performance:** Avoids SQL parsing, query planning, connection pooling, and tabular serialization overhead. Data is written and retrieved directly as raw byte slices.
4. **Copy-on-Write Crash Safety:** Every transaction is atomic, consistent, isolated, and durable (ACID). An abrupt crash or power failure cannot corrupt the database file.

---

## 2. Table Topology & Composite Key Engineering

`redb` operates strictly on byte-slice keys and values. Because B-Trees order keys in lexicographical byte order, query efficiency depends entirely on **composite key design**.

```
redb Database File (~/.poom/data.redb)
├── SPANS Table                 [SpanId (16B) -> Postcard Span Record Blob]
├── TIME_INDEX Table            [(Timestamp_BE (8B) + TraceId (16B)) -> ()]
├── TRACE_SPANS Multimap        [TraceId (16B) -> SpanId (16B)]
├── SPAN_CHILDREN Multimap      [ParentSpanId (16B) -> ChildSpanId (16B)]
├── TAG_INDEX Table             [(TagKey_len + TagKey + TagVal_len + TagVal + TraceId) -> ()]
├── EVALUATIONS Table           [EvaluationId (16B) -> Postcard Evaluation Blob]
└── TRACE_EVALUATIONS Multimap  [TraceId (16B) -> EvaluationId (16B)]
```

---

### 2.1 Table Specifications & Mechanics

#### 1. `SPANS` Table (`Table<&[u8; 16], &[u8]>`):
- **Key:** `SpanId` (16 bytes, raw UUIDv7 bytes).
- **Value:** Serialized `SpanRecord` byte slice (encoded via `postcard`).
- **Access Pattern:** $O(\log N)$ instant point lookup by span ID. Given any span ID, retrieves the full span metadata, attributes, metrics, and logs in under 2 microseconds.

#### 2. `TIME_INDEX` Table (`Table<&[u8; 24], ()>`):
- **Key Layout (24 Bytes):**
  - **Offset `[0..8]`:** `Timestamp_BE` (8 bytes, Big-Endian unsigned 64-bit Unix nanoseconds: `u64::to_be_bytes()`).
  - **Offset `[8..24]`:** `TraceId` (16 bytes).
- **Value:** Zero-sized unit type `()`.
- **The Big-Endian Ordering Advantage:**
  - Little-Endian integers flip byte significance, ruining lexicographical byte comparison.
  - Big-Endian guarantees that higher numbers produce larger byte values (`0x00...01` < `0x00...FF` < `0x01...00`).
  - Lexicographical B-tree ordering corresponds 1:1 with chronological time.
- **Reverse Range Scanning (`.iter().rev()`):**
  - Querying the latest $N$ traces (e.g. for the live UI trace table) performs a backward range scan starting at the end of the B-Tree.
  - Returns the top 50 most recent traces in microseconds without secondary index tables or sorting overhead.
- **Time-Window Range Queries:**
  - Querying traces between time $T_1$ and $T_2$ translates to a bounded B-tree range scan: `[T1_BE + zeros]..=[T2_BE + 0xFFs]`.

#### 3. `TRACE_SPANS` Multimap (`MultimapTable<&[u8; 16], &[u8; 16]>`):
- **Key:** `TraceId` (16 bytes).
- **Value:** `SpanId` (16 bytes).
- **Mechanics:** `redb` multimap tables permit multiple values per key.
- **Access Pattern:** In a single key seek (`table.get(&trace_id)?`), retrieves all associated span IDs belonging to a root trace run, avoiding full table scans.

#### 4. `SPAN_CHILDREN` Multimap (`MultimapTable<&[u8; 16], &[u8; 16]>`):
- **Key:** `ParentSpanId` (16 bytes).
- **Value:** `ChildSpanId` (16 bytes).
- **Mechanics:** Maps each parent span directly to its immediate children.
- **Access Pattern:** Enables instant recursive or iterative reconstruction of nested agent execution trees, retrieval chains, and tool invocations without graph databases or complex joins.

#### 5. `TAG_INDEX` Table (`Table<&[u8], ()>`):
- **Composite Key Layout:**
  - `[2 Bytes]`: Tag key length $K$ (u16 Little-Endian).
  - `[K Bytes]`: Tag key UTF-8 bytes (e.g., `"environment"`, `"model"`).
  - `[2 Bytes]`: Tag value length $V$ (u16 Little-Endian).
  - `[V Bytes]`: Tag value UTF-8 bytes (e.g., `"production"`, `"gpt-4o"`).
  - `[16 Bytes]`: `TraceId`.
- **Value:** `()`.
- **Mechanics:** Prefix range scanning over `[K_len + K + V_len + V]` yields all `TraceId`s carrying that tag.

#### 6. `EVALUATIONS` Table & `TRACE_EVALUATIONS` Multimap:
- Stores `EvaluationRecord` blobs by `EvaluationId` (16 bytes).
- Multimap associates `TraceId` $\rightarrow$ `EvaluationId` for fast retrieval of feedback scores and guardrail assessments alongside traces.

---

## 3. Concurrency Architecture & The Single-Writer / Multi-Reader Model

`redb` enforces strict single-writer exclusivity via OS file locks while allowing unlimited non-blocking concurrent readers.

```
                          ┌───────────────────────────┐
                          │     Poom Storage Engine   │
                          └─────────────┬─────────────┘
                                        │
                 ┌──────────────────────┴──────────────────────┐
                 ▼                                             ▼
  ┌───────────────────────────────┐             ┌──────────────────────────────┐
  │   Ingestion Write Loop        │             │   UI / API Reader Engine     │
  │   (Dedicated Background Task) │             │   (Concurrent Non-Blocking)  │
  └──────────────┬────────────────┘             └──────────────┬───────────────┘
                 │ Holds single                                │ Opens lightweight
                 │ WriteTransaction                            │ ReadTransactions
                 ▼                                             ▼
         ┌─────────────────────────────────────────────────────────────┐
         │                  redb Database (MVCC Engine)                │
         │                  ~/.poom/data.redb                          │
         └─────────────────────────────────────────────────────────────┘
```

### 3.1 The Ingestion Writer Actor
- Writes must never block the Tokio async runtime worker threads or socket listeners.
- The storage writer runs as a dedicated actor/worker thread receiving span batches from an async channel (`flume` or `tokio::sync::mpsc`).
- It is the sole owner of `Database::begin_write()`.

### 3.2 In-Process Concurrent Readers (Iced UI & Queries)
- Desktop UI threads and analytical queries open read transactions (`Database::begin_read()`).
- Readers see an immutable, consistent snapshot of the B-Tree as of the start of their transaction.
- Readers never block the writer loop, and active write transactions never block readers from querying historical spans.

---

## 4. Micro-Batching Flush Engine & Write Coalescing

Calling `write_txn.commit()` incurs a synchronous disk flush (`fsync`). If every span triggered an individual commit, disk I/O would cap throughput at only ~200 spans/second.

### 4.1 Dual Flush Triggers
To sustain **> 50,000 spans/second** while maintaining live UI reactivity, the writer loop coalesces writes into micro-batches with dual triggers:
1. **Batch Size Trigger (High-Throughput Mode):**
   - As soon as the in-memory buffer accumulates **1,000 spans**, the batch commits immediately.
2. **Timeout Trigger (Low-Latency Reactivity Mode):**
   - If fewer than 1,000 spans are in the queue, a timer triggers a commit every **50 milliseconds**.
   - This ensures live spans appear on the user's desktop UI waterfall canvas within 50ms of execution.

### 4.2 Atomic Transaction Batching
Inside a single write transaction:
1. For each span in the batch:
   - Insert span into `SPANS`.
   - If the span is a root span (`parent_span_id.is_none()`), insert `(start_time_BE, trace_id)` into `TIME_INDEX`.
   - Insert `(trace_id, span_id)` into `TRACE_SPANS`.
   - If `parent_span_id` is present, insert `(parent_id, span_id)` into `SPAN_CHILDREN`.
   - For indexed attributes, insert into `TAG_INDEX`.
2. Commit the transaction atomically in a single disk sync.
3. If an unexpected error or power loss occurs before `commit()`, all changes roll back completely—no orphaned spans or corrupted index pointers can exist.

---

## 5. Retention Pruning & Data Compaction (TTL Engine)

Observability databases grow continuously. Without automated pruning, developer disks would eventually fill up.

### 5.1 The Pruning Algorithm
A periodic background maintenance task runs every hour (or upon manual CLI trigger):
1. **Determine Cutoff:** Computes the cutoff timestamp: `cutoff_nanos = now - retention_period` (e.g. 7 days or 30 days).
2. **Forward Scan `TIME_INDEX`:**
   - Scans `TIME_INDEX` starting from the beginning of the B-tree (`0`) up to `cutoff_nanos`.
   - Extracts all expired `TraceId`s.
   - Halts scanning immediately upon encountering the first timestamp $\ge$ `cutoff_nanos`.
3. **Cascading Deletion:**
   - For each expired `TraceId`:
     - Looks up all `SpanId`s in `TRACE_SPANS`.
     - Deletes all matching entries from `SPANS`.
     - Deletes parent-child pointers from `SPAN_CHILDREN`.
     - Deletes tags from `TAG_INDEX`.
     - Deletes evaluations from `EVALUATIONS` and `TRACE_EVALUATIONS`.
     - Deletes the `TIME_INDEX` entry itself.
4. **Single Maintenance Transaction:**
   - Commits deletions in a single transaction, releasing disk blocks back to `redb`'s internal free-page list for immediate reuse by future writes.

---

## 6. Defensive Guardrails & Edge Cases

1. **Database File Locking:** `redb` uses OS-level file locking. If a second Poom process attempts to open the same database file in read-write mode, it receives a clear `DatabaseAlreadyOpen` error rather than corrupting data.
2. **Crash Recovery & Rollback:** Copy-on-Write B-Trees ensure that incomplete writes never touch active database pages. Recovery on startup is instantaneous ($< 10$ milliseconds) with zero journal replay lag.
3. **Database Directory Initialization:** Automatically creates parent storage directories (`~/.poom/`) with restricted POSIX permissions (`0700`).
4. **Graceful Shutdown & Drain:** On application termination, the write loop drains all remaining buffered spans from the channel, executes a final commit, and cleanly closes the database file.

---

## 7. Verification, Integration & Benchmarking Plan

### 7.1 Integration Test Matrix
1. **Point Lookups & Tree Reconstruction:**
   - Insert an agent workflow with 1 root span, 2 chain spans, 4 LLM spans, and 4 tool spans.
   - Verify that querying by `TraceId` extracts all 11 spans.
   - Verify that traversing `SPAN_CHILDREN` reconstructs the exact directed execution tree.
2. **Reverse Chronological Pagination:**
   - Insert 100 traces with strictly increasing timestamps.
   - Request page 1 (limit 20). Verify traces arrive in reverse chronological order (newest first).
3. **Micro-Batch Flush Verification:**
   - Push 5,000 spans across multiple concurrent threads into the storage engine.
   - Verify that all 5,000 spans are durable and queryable.
4. **TTL Pruning Verification:**
   - Insert 50 traces with timestamps 10 days in the past, and 50 traces with timestamps 1 hour in the past.
   - Run pruner with 7-day retention.
   - Verify the 50 older traces are completely erased from all 5 tables while the 50 newer traces remain fully intact.
5. **Simulated Crash & Atomic Rollback:**
   - Start a write transaction, insert partial records, and drop the transaction without committing.
   - Verify that zero records leak into the database.

### 7.2 Performance & Benchmarking Targets
- **Batch Ingestion Speed:** Commit 1,000 spans in under **10 milliseconds** (< 10 μs per span).
- **Recent Traces Page Query:** Retrieve the top 50 recent traces from a database of 100,000 spans in under **1.0 millisecond**.
- **Full Trace Tree Extraction:** Retrieve and assemble a 500-span multi-agent trace hierarchy in under **2.0 milliseconds**.
