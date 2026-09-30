# Phase 7: Analytics, Prompt Diffing, Unified CLI & Packaging
## Detailed Architectural & Mechanical Specification

---

## 1. Scope, Purpose & Architectural Boundary

Phase 7 represents the capstone phase of the Poom ecosystem. It unifies all previous subsystems—the universal taxonomy (`poom-types`), binary wire protocol (`poom-protocol`), cross-platform IPC transport (`poom-transport`), embedded B-Tree storage (`poom-storage`), ingestion daemon (`poom-daemon`), native desktop studio (`poom-ui`), and Python instrumentation bridge (`poom-py`)—into a **single, self-contained, production-grade binary and distribution suite**.

### 1.1 The Operational Fragmentation Problem
Traditional observability solutions suffer from severe deployment friction:
1. **Container & Infrastructure Sprawl:** Running Langfuse or OpenTelemetry requires managing Docker Compose files, PostgreSQL databases, ClickHouse clusters, Redis instances, and separate web frontend nodes.
2. **Disconnected Tooling:** Comparing prompt changes, extracting trace data for evaluation datasets, and profiling cost trends typically require bouncing between web dashboards, Python notebooks, and ad-hoc SQL queries.
3. **Headless vs. GUI Duality:** Developers want a fluid 60 FPS GPU-accelerated desktop UI during local prompt engineering, but need a lightweight, headless daemon in CI/CD pipelines, Docker containers, and remote dev servers.

### 1.2 The Poom Capstone Architecture
Phase 7 introduces **`crates/poom-cli`** and integrates advanced analytical workflows:
- **Unified Binary Entrypoint:** A single executable (`poom`) that seamlessly switches between desktop GUI mode, headless background daemon, storage maintenance, and data export.
- **Visual Prompt Diff Engine:** Line- and token-level visual diffing using `similar` to track prompt drift, temperature variances, and system prompt iterations.
- **Cost & Latency Analytics Engine:** Time-series percentile latency metrics ($p_{50}$, $p_{90}$, $p_{99}$), model token velocity charts, and provider cost trajectory tracking.
- **Automated Data Maintenance & Portable Export:** Built-in retention pruning (`poom prune`) and bidirectional data export to standard JSON and OpenTelemetry OTLP format (`poom export`).
- **Universal Zero-Dependency Distribution:** Native static binaries compiled with Cargo/cross, alongside a single unified Python wheel packaged via Maturin for Python 3.9 through 3.13.

```text
                               ┌─────────────────────────────────────────────────────────────┐
                               │                    Unified CLI Binary                       │
                               │                        (poom)                               │
                               └──────────────────────────────┬──────────────────────────────┘
                                                              │
                                 ┌────────────────────────────┼────────────────────────────┐
                                 │ clap 4 Mode Dispatch       │                            │
                                 ▼                            ▼                            ▼
                 ┌──────────────────────────────┐ ┌───────────────────────┐ ┌──────────────────────────────┐
                 │         poom studio          │ │      poom daemon      │ │         poom prune           │
                 │   (Desktop GUI + Daemon)     │ │   (--headless server) │ │   (TTL Storage Compactor)    │
                 └──────────────┬───────────────┘ └───────────┬───────────┘ └──────────────┬───────────────┘
                                │                             │                            │
                                ├─────────────────────────────┴────────────────────────────┤
                                ▼                                                          ▼
                 ┌──────────────────────────────┐                           ┌──────────────────────────────┐
                 │         poom export          │                           │          poom diff           │
                 │   (JSON & OTLP Exporter)     │                           │   (Prompt & Output Diffing)  │
                 └──────────────────────────────┘                           └──────────────────────────────┘
```

---

## 2. Unified CLI Architecture & Command Topology

The unified CLI binary is built with `clap = { version = "4.5", features = ["derive", "env"] }`.

### 2.1 Command Grammar & Flags

```text
poom [OPTIONS] [COMMAND]

COMMANDS:
  studio    Launch the native GPU desktop studio and embedded daemon [default]
  daemon    Run the headless background ingestion daemon and storage engine
  prune     Reclaim disk space by purging traces older than retention cutoff
  export    Export trace hierarchies to standard JSON or OpenTelemetry OTLP
  diff      Compute visual or terminal diff between two spans, prompts, or traces
  stats     Display quick storage, token, and cost metrics summary in the terminal

OPTIONS:
  -c, --config <PATH>       Path to custom configuration file [default: ~/.poom/config.toml]
  -d, --db <PATH>           Path to redb database file [default: ~/.poom/data.redb]
  -s, --socket <PATH>       Path to IPC domain socket [default: $XDG_RUNTIME_DIR/poom/poom.sock]
  -v, --verbose             Enable debug telemetry logging
  -h, --help                Print help information
  -V, --version             Print version information
```

### 2.2 Subcommand Specifications

#### 1. `poom studio` (Default Interactive Mode)
- **Invocation:** `poom` or `poom studio`.
- **Behavior:**
  1. Resolves `$HOME/.poom` directory and database path (`data.redb`).
  2. Initializes a lightweight 2-thread Tokio runtime.
  3. Boots the `PoomDaemon` listening on `/tmp/poom.sock` (or `$XDG_RUNTIME_DIR/poom/poom.sock`).
  4. Checks storage state; if empty, optionally seeds demo traces for immediate user onboarding.
  5. Spawns the socket listener on a background Tokio task.
  6. Launches the native Iced GUI (`poom-ui`) on the main OS thread with the shared storage handle and live event broadcaster.
  7. When the GUI window is closed by the user, intercepts window close, signals daemon shutdown, drains pending micro-batches to `redb`, removes the socket file, and terminates cleanly with exit code `0`.

#### 2. `poom daemon` (`poom --headless`)
- **Invocation:** `poom daemon` or `poom --headless`.
- **Behavior:**
  1. Runs purely in headless mode with **zero display/X11/Wayland/GPU dependencies**.
  2. Spawns a Tokio runtime configured for production server workloads (2 worker threads, idle RSS < 15 MB).
  3. Binds the IPC socket (`IpcListener`) and opens `StorageEngine` in read-write mode.
  4. Registers OS signal handlers for `SIGINT` (Ctrl+C) and `SIGTERM`.
  5. Logs incoming client connections, batch commits, and throughput metrics to standard output.
  6. Upon receiving a termination signal:
     - Stops accepting new client connections.
     - Awaits in-flight frame processing.
     - Commits the current micro-batch in `BatchWriter`.
     - Unlinks the socket file and exits cleanly.

#### 3. `poom prune`
- **Invocation:** `poom prune --days <N> [--dry-run]`
- **Arguments:**
  - `--days <N>`: Retention threshold in days (e.g. `--days 14`). Traces with root timestamp $< (\text{now} - N \times 86400\text{s})$ are purged.
  - `--dry-run`: Scans `TIME_INDEX` and calculates how many traces and spans would be deleted, without mutating the database.
- **Output:**
  ```text
  Scanned 14,200 traces in 4.2ms.
  Pruned 3,120 traces (18,450 spans) older than 14 days.
  Reclaimed estimated 14.8 MB of B-Tree disk pages.
  ```

#### 4. `poom export`
- **Invocation:** `poom export --trace <TRACE_ID> [--format <json|otlp>] [--output <FILE>]`
- **Arguments:**
  - `--trace <TRACE_ID>`: 128-bit UUIDv7 trace identifier.
  - `--since <DURATION>`: Optional duration filter (e.g. `--since 24h` or `--since 7d`) to export a batch of traces.
  - `--format <json|otlp>`: Output format. Default is `json`.
  - `--output <FILE>`: Destination file path (defaults to stdout if omitted).
- **Behavior:**
  - Opens `redb` in read-only mode (can run concurrently with an active daemon).
  - Traverses `TRACE_SPANS` and `SPAN_CHILDREN` to assemble full directed execution trees.
  - Formats tree into structured JSON or OTLP v1 Protobuf/JSON schema.

#### 5. `poom diff`
- **Invocation:** `poom diff --span1 <SPAN_ID_1> --span2 <SPAN_ID_2> [--field <prompt|completion|all>]`
- **Behavior:**
  - Retrieves both spans from `redb`.
  - Extracts prompt or completion strings from attributes.
  - Computes word-level and line-level diffs via the `similar` engine.
  - Emits colorized ANSI terminal output highlighting additions and deletions.

#### 6. `poom stats`
- **Invocation:** `poom stats`
- **Behavior:**
  - Queries `StorageReader` for summary metrics:
    - Total stored traces and total spans.
    - Total tokens consumed (input, output, cached).
    - Total estimated financial cost in USD.
    - Global success vs error ratio.
    - Database file size on disk and page free-space ratio.
  - Emits a clean ASCII summary table to stdout in $< 5\text{ms}$.

---

## 3. Visual Prompt Diff Engine (`diff/prompt_diff.rs`)

Prompt engineering requires tracking minute variations across iterations, understanding model output behavioral changes across temperature adjustments, and detecting system prompt drift.

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                 Visual Prompt Diff Viewer                              │
│  Span A: 019234a1 (gpt-4o · T=0.2)           Span B: 019234b8 (gpt-4o · T=0.7)        │
├──────────────────────────────────────────────┬─────────────────────────────────────────┤
│  Line 1: You are a senior equity analyst.   │  Line 1: You are a senior equity analyst.│
│ -Line 2: Provide a concise 3-bullet summary. │ +Line 2: Provide an exhaustive breakdown.│
│  Line 3: Focus on gross margins and risks.   │  Line 3: Focus on gross margins and risks.│
│                                              │ +Line 4: Include projected FY25 targets.│
├──────────────────────────────────────────────┴─────────────────────────────────────────┤
│ Summary: 1 deletion (-), 2 additions (+) · Similarity Score: 84.6%                     │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.1 Algorithm & Diff Mechanics
1. **Myers / Patience Diffing via `similar`:**
   - The engine utilizes the `similar` crate (`similar = "2.6"`), supporting LCS (Longest Common Subsequence) and Myers algorithms.
   - Computes structural line-level differences first to align common blocks.
2. **Two-Tier Diff Resolution:**
   - **Tier 1 (Line-Level Diff):** Identifies added, deleted, and unchanged lines (`ChangeTag::Insert`, `ChangeTag::Delete`, `ChangeTag::Equal`).
   - **Tier 2 (Word/Token-Level Sub-Diff):** For lines marked as modified (a deletion immediately adjacent to an insertion), the engine performs a secondary word-level sub-diff.
   - Highlights the exact words or tokens changed inside the line rather than invalidating the entire line.
3. **Similarity Index Computation:**
   - Computes the Ratcliff/Obershelp or Levenshtein similarity metric ($0.0 \dots 1.0$):
     $$\text{Similarity Score} = \frac{2 \times M}{T_A + T_B}$$
     Where $M$ is the count of matched tokens, and $T_A, T_B$ are the total token lengths.
4. **Multi-Target Diffing:**
   - **System Prompts:** Detect modifications in base agent persona and guardrail instructions across deployments.
   - **User Input:** Analyze user query variations that triggered model regressions or tool failure.
   - **Model Output / Completion:** Compare responses generated across different model versions (e.g. `gpt-4o` vs `claude-3-5-sonnet`) or temperature settings for identical inputs.

### 3.2 Dual Presentation Surfaces
1. **Terminal Surface (CLI):**
   - Renders with ANSI color escapes:
     - Insertions: Bold Green (`\x1b[32m+\x1b[0m`).
     - Deletions: Bold Red (`\x1b[31m-\x1b[0m`).
     - Line Numbers: Monospace Muted Slate (`\x1b[90m`).
     - Intraline Word Accents: Inverted background highlights.
2. **Desktop Studio Surface (GUI Widget in `poom-ui`):**
   - Implements a dedicated `view_prompt_diff` widget.
   - Offers toggleable **Side-by-Side** (split column) and **Unified Inline** rendering.
   - Uses curated semantic colors:
     - Deletion background: Crimson Tint (`#450A0A` at 60% opacity) with bright red strikethrough.
     - Insertion background: Emerald Tint (`#064E3B` at 60% opacity) with bright green text.
     - Unchanged lines: Muted slate foreground (`#94A3B8`).

---

## 4. Metrics & Cost Analytics Dashboard (`analytics/`)

While the Gantt waterfall canvas in `poom-ui` provides micro-level forensic analysis of individual traces, developers also need macro-level analytical insight into system health, cost trajectories, and latency degradation over time.

### 4.1 Metric Aggregations & Query Pipeline

The analytics engine processes historical spans in `redb` across time windows ($15\text{m}, 1\text{h}, 24\text{h}, 7\text{d}, 30\text{d}$):

```text
                               ┌─────────────────────────────┐
                               │   Analytics Query Engine    │
                               └──────────────┬──────────────┘
                                              │ Scans TIME_INDEX
                                              ▼
                               ┌─────────────────────────────┐
                               │     Time-Bucketed Bins      │
                               │  (e.g., 60-second intervals)│
                               └──────────────┬──────────────┘
                                              │
                 ┌────────────────────────────┼────────────────────────────┐
                 ▼                            ▼                            ▼
   ┌───────────────────────────┐┌───────────────────────────┐┌───────────────────────────┐
   │    Latency Percentiles    ││      Token Velocity       ││      Cost Trajectory      │
   │  p50, p90, p95, p99 (ms)  ││ Prompt, Completion, Cached││    USD Accumulation ($)   │
   └───────────────────────────┘└───────────────────────────┘└───────────────────────────┘
```

#### 1. Latency Percentiles ($p_{50}, p_{90}, p_{95}, p_{99}$)
- Spans are partitioned into chronological time buckets (e.g. 60-second intervals for a 1-hour view; 1-hour intervals for a 7-day view).
- In each bucket, span durations are collected into an in-memory histogram / quantile estimator (`hdrhistogram` or sorted vector).
- Computes exact $p_{50}$ (median), $p_{90}$, $p_{95}$, and $p_{99}$ latency thresholds in nanoseconds.

#### 2. Token Velocity & Volume by Model
- Tracks token consumption categorized by model family (`gpt-4o`, `claude-3-5-sonnet`, `gemini-1.5-flash`, etc.).
- Bins metrics across:
  - Input / Prompt tokens.
  - Output / Completion tokens.
  - Cache-read tokens (quantifying cache savings).
  - Reasoning tokens (OpenAI o1/o3 series).

#### 3. Cumulative Cost Trajectory (USD)
- Aggregates the `estimated_cost_usd` field across all LLM spans over the selected time range.
- Plots both the instant burn rate ($/hour) and the cumulative total cost curve ($).

#### 4. Success vs. Error Rate Histograms
- Stacks successful spans against failed spans per time bucket.
- Categorizes errors by classification (e.g., `RateLimitError`, `TimeoutException`, `ValidationError`).

### 4.2 Chart Rendering Architecture (`plotters` & `plotters-iced`)

1. **Native GPU / Canvas Plotting:**
   - Uses `plotters` (`plotters = "0.3"`) and `plotters-iced` (`plotters-iced = "0.13"`).
   - Renders directly into Iced's canvas rendering pipeline, avoiding web-view wrappers or external image export.
2. **Curated Dark Theme Chart Styling:**
   - Gridlines: Subtle slate borders (`#1E293B`).
   - Latency $p_{50}$ Line: Cyan (`#06B6D4`).
   - Latency $p_{99}$ Line: Amber (`#F59E0B`).
   - Cost Curve: Emerald Green (`#10B981`).
   - Error Bars: Crimson Red (`#EF4444`).
3. **Responsive Chart Layout:**
   - Dynamically scales to match window dimensions and HiDPI display factors.
   - Interactive hover points displaying exact value tooltips at mouse cursor coordinates.

---

## 5. Portable Data Export Engine (`export/`)

To support fine-tuning dataset generation, automated evaluation pipelines, and compliance archiving, Poom provides high-speed, portable trace export.

### 5.1 Export Schemas

#### 1. Standard Hierarchical JSON (`Format::Json`)
Exports traces as clean, self-contained JSON trees:
```json
{
  "version": "1.0",
  "exported_at_unix_nanos": 1727670000000000000,
  "trace": {
    "trace_id": "019234a1-89c0-7000-8000-000000000001",
    "root_span": {
      "span_id": "019234a1-89c0-7000-8000-000000000002",
      "name": "FinancialAnalystAgent",
      "kind": "Agent",
      "start_time_unix_nanos": 1727669820000000000,
      "end_time_unix_nanos": 1727669821820000000,
      "duration_ms": 1820.0,
      "status": { "state": "Ok" },
      "attributes": {
        "workflow.type": "financial_analyst_v2",
        "user.id": "usr_4920"
      },
      "metrics": {
        "input_tokens": 1450,
        "output_tokens": 320,
        "estimated_cost_usd": 0.0215
      },
      "children": [
        {
          "span_id": "019234a1-89c0-7000-8000-000000000003",
          "name": "RetrieveSECFilings",
          "kind": "Tool",
          "duration_ms": 160.0,
          "status": { "state": "Ok" },
          "attributes": { "tool.name": "sec_edgar_retriever" },
          "children": []
        },
        {
          "span_id": "019234a1-89c0-7000-8000-000000000004",
          "name": "LlmCall:gpt-4o",
          "kind": "Llm",
          "duration_ms": 1520.0,
          "status": { "state": "Ok" },
          "attributes": {
            "gen_ai.request.model": "gpt-4o",
            "prompt": "Analyze Apple Q3 10-K data...",
            "completion": "Executive Summary: Apple's Q3 results..."
          },
          "children": []
        }
      ]
    },
    "evaluations": [
      {
        "name": "factual_accuracy",
        "value": 0.98,
        "comment": "All figures matched official SEC filing tables."
      }
    ]
  }
}
```

#### 2. OpenTelemetry OTLP Compatible Schema (`Format::Otlp`)
Exports spans formatted according to OpenTelemetry Trace Specification v1:
- Maps `TraceId` and `SpanId` to standard hex-encoded 16-byte and 8-byte IDs.
- Maps `SpanKind` to standard OTel `SpanKind` numbers (`INTERNAL`, `SERVER`, `CLIENT`).
- Injects standard semantic conventions: `gen_ai.system`, `gen_ai.request.model`, `gen_ai.usage.prompt_tokens`, `gen_ai.usage.completion_tokens`.
- Can be directly piped or ingested by downstream enterprise tools (Jaeger, Datadog, Honeycomb, ClickHouse).

---

## 6. Packaging, Cross-Compilation & Distribution

Poom is distributed through two primary channels:
1. **The Native Standalone CLI Binary:** Single executable containing daemon, storage, GUI, and CLI tools.
2. **The Python Package (`poom`):** Wheel containing the compiled PyO3 native extension, pure Python client SDK, and CLI entrypoint.

### 6.1 Rust Static Binary Packaging

- **Release Profile Optimizations (`Cargo.toml`):**
  ```toml
  [profile.release]
  opt-level = 3
  lto = "fat"
  codegen-units = 1
  panic = "abort"
  strip = true
  ```
- **Target Matrices:**
  - `x86_64-unknown-linux-gnu` / `x86_64-unknown-linux-musl` (Linux x86_64 static binary)
  - `aarch64-unknown-linux-gnu` (Linux ARM64 / Graviton / Raspberry Pi)
  - `x86_64-apple-darwin` (macOS Intel)
  - `aarch64-apple-darwin` (macOS Apple Silicon M1/M2/M3/M4)
  - `x86_64-pc-windows-msvc` (Windows 10/11 x64)
- **GUI Dependency Isolation:**
  - `poom-cli` configures feature flags:
    - `default = ["gui"]`: Includes `poom-ui`, `wgpu`, and windowing libraries.
    - `--no-default-features --features headless`: Builds a featherweight headless daemon without X11, Wayland, or GPU libraries (~5 MB binary), ideal for minimal Docker containers.

### 6.2 Python Maturin Wheel Packaging

- **Maturin Build Workflow (`pyproject.toml`):**
  - Targets Python Stable ABI (`abi3-py39`).
  - Single wheel per OS architecture functions across Python 3.9, 3.10, 3.11, 3.12, and 3.13.
- **Python CLI Wrapper:**
  - `python/poom/pyproject.toml` registers console scripts:
    ```toml
    [project.scripts]
    poom = "poom.cli:main"
    ```
  - Allows developers who run `pip install poom` to immediately run `poom studio` or `poom daemon` from their terminal without installing Rust.

---

## 7. Crate Architecture & Deliverables in `crates/poom-cli`

Phase 7 introduces the `crates/poom-cli` crate and updates workspace dependencies:

```text
poom/
├── Cargo.toml                         # Adds crates/poom-cli to workspace members
└── crates/
    └── poom-cli/
        ├── Cargo.toml                 # Dependencies: clap, similar, plotters, poom-*, tokio
        └── src/
            ├── main.rs                # Entrypoint, clap CLI argument parsing, mode dispatch
            ├── error.rs               # CliError and Result definitions
            │
            ├── commands/              # Subcommand implementations
            │   ├── mod.rs             # Subcommand router
            │   ├── studio.rs          # GUI studio runner (wraps poom-ui + daemon)
            │   ├── daemon.rs          # Headless background server runner
            │   ├── prune.rs           # Storage retention compaction
            │   ├── export.rs          # JSON and OTLP trace export
            │   ├── diff.rs            # Terminal prompt & completion diffing
            │   └── stats.rs           # Terminal summary telemetry reporter
            │
            ├── diff/                  # Visual and terminal diff engine
            │   ├── mod.rs
            │   ├── engine.rs          # Myers/LCS diff algorithm using `similar`
            │   └── terminal.rs        # ANSI terminal highlighter
            │
            ├── analytics/             # Time-series analytics & metrics
            │   ├── mod.rs
            │   ├── metrics.rs         # Percentile latency and token velocity accumulators
            │   └── time_series.rs     # Binned time-window aggregation
            │
            └── export/                # Format serialization
                ├── mod.rs
                ├── json.rs            # Tree JSON serializer
                └── otlp.rs            # OpenTelemetry trace model mapping
```

### 7.1 Key Struct & Enum Definitions

```rust
// crates/poom-cli/src/commands/mod.rs

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "poom", author, version, about = "Poom: Pure-Rust Observability Studio", long_about = None)]
pub struct Cli {
    #[arg(short, long, global = true, help = "Custom redb database path")]
    pub db: Option<PathBuf>,

    #[arg(short, long, global = true, help = "Custom IPC domain socket path")]
    pub socket: Option<PathBuf>,

    #[arg(short, long, global = true, help = "Enable verbose debug logging")]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Launch the native desktop studio and embedded daemon [default]")]
    Studio,

    #[command(about = "Run the headless ingestion daemon and storage engine")]
    Daemon {
        #[arg(long, default_value = "false", help = "Disable background retention pruner")]
        no_prune: bool,
    },

    #[command(about = "Purge historical traces older than retention cutoff")]
    Prune {
        #[arg(short, long, default_value = "14", help = "Retention threshold in days")]
        days: u32,

        #[arg(long, help = "Calculate purgable records without deleting")]
        dry_run: bool,
    },

    #[command(about = "Export trace hierarchies to JSON or OTLP format")]
    Export {
        #[arg(short, long, help = "Trace ID (UUIDv7) to export")]
        trace: Option<String>,

        #[arg(long, help = "Export traces created within duration (e.g. 24h, 7d)")]
        since: Option<String>,

        #[arg(short, long, default_value = "json", help = "Export format: json or otlp")]
        format: ExportFormat,

        #[arg(short, long, help = "Output destination file path [default: stdout]")]
        output: Option<PathBuf>,
    },

    #[command(about = "Compute visual diff between two spans, prompts, or traces")]
    Diff {
        #[arg(long, help = "First Span ID (UUIDv7)")]
        span1: String,

        #[arg(long, help = "Second Span ID (UUIDv7)")]
        span2: String,

        #[arg(long, default_value = "prompt", help = "Field to diff: prompt, completion, or all")]
        field: DiffField,
    },

    #[command(about = "Display summary metrics and database storage stats")]
    Stats,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Otlp,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffField {
    Prompt,
    Completion,
    All,
}
```

---

## 8. Verification, Acceptance Criteria & End-to-End Test Plan

To validate Phase 7 completion, the deliverables will satisfy the following concrete test criteria:

### 8.1 Acceptance Criteria Matrix

| Subsystem | Requirement | Target Metric |
| :--- | :--- | :--- |
| **Unified CLI** | Default invocation (`poom`) launches desktop studio | Opens GUI in $< 500\text{ms}$; begins listening on IPC socket |
| **Headless Daemon** | `poom daemon` executes without GUI/X11 linkages | Runs cleanly in headless Linux Docker/CI with $< 15\text{MB}$ RSS |
| **Retention Pruner** | `poom prune --days N` executes cascading deletions | Purges 10,000 expired spans across all 5 tables in $< 50\text{ms}$ |
| **Data Export** | `poom export --trace <ID>` outputs valid JSON | Emits full hierarchical trace tree with 100% roundtrip fidelity |
| **Prompt Diff** | `poom diff` highlights word- and line-level changes | Accurate Myers diff highlighting insertions/deletions with ANSI |
| **Cross-Platform** | Single binary distribution | Builds cleanly under `cargo build --release` on Linux, macOS, Windows |

### 8.2 End-to-End Integration Test (`tests/e2e_integration_test.rs`)
An end-to-end integration test orchestrating the complete lifecycle:
1. Initialize a temporary test directory with custom socket and `redb` paths.
2. Launch `poom daemon` in a background Tokio task.
3. Use the Python SDK client or `poom-transport` to ingest 25 multi-span traces (including LLM inferences, tool calls, and error states).
4. Run `poom stats` and verify span counts and token accumulators match ingested numbers.
5. Run `poom diff` on two seeded LLM spans with differing prompts; assert diff captures the modified tokens.
6. Run `poom export --format json` on a seeded trace; assert output parses as valid JSON with all child spans present.
7. Run `poom prune --days 0`; verify all records are safely cleared from `redb`.
8. Shutdown daemon cleanly and verify socket file is removed.

---

## 9. Implementation Roadmap & Execution Checklist

1. **Step 1: Workspace Wiring & `crates/poom-cli` Setup**
   - Create `crates/poom-cli/Cargo.toml` with dependencies (`clap`, `similar`, `poom-types`, `poom-protocol`, `poom-transport`, `poom-storage`, `poom-daemon`, `poom-ui`, `tokio`).
   - Register `crates/poom-cli` in workspace root `Cargo.toml`.
2. **Step 2: CLI Core & Subcommands**
   - Implement `main.rs`, `Cli` argument parsing, and command dispatch.
   - Implement `commands/daemon.rs` with signal trapping (`SIGINT`, `SIGTERM`).
   - Implement `commands/studio.rs` connecting embedded daemon + Iced studio.
   - Implement `commands/stats.rs` querying storage stats.
3. **Step 3: Storage Pruning & Export**
   - Implement `commands/prune.rs` integrating with `TTLPruner`.
   - Implement `commands/export.rs` with `export/json.rs` and `export/otlp.rs`.
4. **Step 4: Prompt Diff Engine**
   - Implement `diff/engine.rs` wrapping `similar` for line- and token-level diffs.
   - Implement `diff/terminal.rs` with ANSI color formatting.
   - Wire diff viewer into `commands/diff.rs`.
5. **Step 5: End-to-End Tests & Verification**
   - Implement `crates/poom-cli/tests/e2e_cli_test.rs`.
   - Validate workspace compilation with `cargo check --workspace`.
   - Run complete test suite with `cargo test --workspace`.
   - Verify Python test suite passes.
