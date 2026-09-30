# Phase 7: Analytics, Prompt Diffing, CLI & Packaging

## 1. Objective
Bring all subsystems together into a unified, production-grade distribution. Implement visual prompt comparison, latency and cost analytics charts, a versatile dual-mode CLI binary, and the Python package distribution workflow via `maturin`.

---

## 2. Key Features & Subsystems

### 2.1 Prompt Diff Engine (`similar`)
- Side-by-side or inline visual diffing comparing:
  - System prompts across different trace runs.
  - Prompt variations during prompt engineering experiments.
  - Model outputs for identical inputs across temperature settings or model versions.
- Uses `similar` to compute word/line-level character diffs with green/red syntax highlighting.

### 2.2 Metrics & Cost Analytics Dashboard (`plotters` & `plotters-iced`)
- Time-series charts embedded directly into the Iced desktop studio:
  - Latency percentiles (p50, p90, p99) over selected time ranges.
  - Cumulative and rolling token consumption by model.
  - Cumulative cost trajectory ($) broken down by model provider.
  - Success vs Error rate histograms.

### 2.3 Unified CLI Binary (`crates/poom-cli`)
- Built with `clap = { version = "4", features = ["derive"] }`.
- **Modes:**
  - `poom` / `poom studio`: Launches the desktop GUI while starting the embedded daemon and storage engine in background threads.
  - `poom daemon` / `poom --headless`: Runs exclusively the IPC listener, ingestion pipeline, and `redb` storage engine. Ideal for remote Linux servers, Docker containers, or headless CI environments.
  - `poom prune --days 14`: Manually triggers storage compaction and retention purging.
  - `poom export --trace <ID> --format json`: Exports a trace hierarchy to standard JSON or OpenTelemetry OTLP format.

### 2.4 Packaging & Distribution
- **Rust Binary:** Single self-contained binary built via `cargo build --release` (or `cross` for multi-architecture cross-compilation: Linux x86_64/aarch64, macOS Apple Silicon/Intel, Windows x64).
- **Python Wheel:** Packaged via `maturin` targeting Python Stable ABI (`abi3`), published to PyPI or installable locally via `pip install .`.

---

## 3. Deliverables in `crates/poom-cli` & Integration

- `cli/main.rs`: CLI argument parsing and mode dispatch.
- `cli/daemon_mode.rs`: Headless daemon runner with signal handling (`SIGINT`, `SIGTERM`).
- `cli/studio_mode.rs`: Simultaneous launch of daemon thread + Iced GUI loop.
- `analytics/charts.rs`: Plotters chart rendering pipelines.
- `diff/prompt_diff.rs`: Similar diff algorithm wrapper and visual highlighter.
- `tests/e2e_integration_test.rs`: Full end-to-end integration test: Python app -> IPC -> Ingestion -> Storage -> Query.

---

## 4. Verification & Acceptance Criteria
- [ ] End-to-end test succeeds: Spans emitted by a FastAPI test script appear intact in `redb` and are queryable.
- [ ] CLI runs flawlessly in both GUI mode and `--headless` mode.
- [ ] Sustained load test: Ingest 100,000 spans at 50,000 spans/sec; verify total memory remains < 120 MB RSS.
- [ ] Prompt diff viewer accurately highlights inserted, deleted, and modified prompt segments.
- [ ] Maturin wheel builds cleanly for Python 3.9+ without requiring a local Rust toolchain for end users.
