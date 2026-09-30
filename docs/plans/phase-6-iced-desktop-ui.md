# Phase 6: Iced Desktop Interface (The Observability Studio)

## 1. Objective
Build a native, lightning-fast desktop observability application using `Iced` (implementing The Elm Architecture - TEA). The UI queries `redb` directly in-process and renders complex trace execution trees, 2D Gantt waterfalls, and detailed span inspectors without web browsers, electron bloat, or HTTP/DOM overhead.

---

## 2. Architecture & Design

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ Top Bar: Search Bar | Time Window (15m, 1h, 24h, Custom) | Project/Env Filter | Stats  │
├───────────────────────────────┬────────────────────────────────────────────────────────┤
│ Live Trace List (Table)       │ Trace Detail & Waterfall (Canvas)                      │
│ ───────────────────────────── │ ────────────────────────────────────────────────────── │
│ TraceId   Name   Dur.   Cost  │ ┌─ Root Agent Loop (1.4s) ───────────────────────────┐ │
│ 7f3b...  Agent   1.4s  $0.02  │ │  ├─ Context Search (120ms)                         │ │
│ a12e...  Tool    45ms  $0.00  │ │  └─ LLM Call: gpt-4o (1.28s)                       │ │
│ b98c...  HTTP   210ms  $0.00  │ └────────────────────────────────────────────────────┘ │
│                               ├────────────────────────────────────────────────────────┤
│                               │ Span Inspector (JSON / Prompt / Diff)                  │
│                               │ Model: gpt-4o | Tokens: 840 in / 120 out | Cost: $0.01 │
│                               │ Input: "Calculate quarterly yield..."                  │
│                               │ Output: "Based on the Q3 reports..."                   │
└───────────────────────────────┴────────────────────────────────────────────────────────┘
```

### 2.1 The Elm Architecture (TEA) State Flow
- **Model:** Stores loaded traces, active selection, time range, search filters, zoom/scroll state, and inspector panel data.
- **Message:** `TraceSelected(TraceId)`, `SpanSelected(SpanId)`, `TimeRangeChanged(TimeRange)`, `FilterUpdated(String)`, `LiveSpanReceived(SpanRecord)`, `CanvasScrolled(f32)`.
- **Update:** Updates UI state, queries `redb` read transactions asynchronously, and dispatches UI re-renders.
- **View:** Declarative widget tree composed of split panes, tables, canvases, and metadata inspectors.
- **Subscription:** Listens to the daemon's internal broadcast channel for real-time span events.

### 2.2 Core Views & Screens

1. **Live Trace Explorer:**
   - Real-time tabular list of traces.
   - Columns: Status Indicator (Ok / Error), Name, Kind, Start Time, Latency (ms/s), Token Usage, Estimated Cost.
   - Virtualized scrolling for smooth 60fps rendering across 10,000+ traces.

2. **2D Gantt Waterfall Canvas (`iced::widget::canvas::Program`):**
   - High-performance custom 2D rendering.
   - Transforms hierarchical spans into horizontal duration bars proportional to total trace time.
   - Color coding by `SpanKind`:
     - Purple: `Agent`
     - Green: `Llm`
     - Amber: `Tool`
     - Blue: `Chain`
     - Slate: `Function`
   - Interactive hit-testing: Hover tooltips, click selection, pan & zoom along the timeline.

3. **Span Inspector:**
   - Multi-tab panel:
     - **Overview:** Timing, status, error backtrace, model attributes.
     - **Inputs & Outputs:** Formatted JSON or rendered Markdown view of prompts and responses.
     - **Raw Metadata:** Complete attribute key-value table.

### 2.3 Rendering Engines
- Default: `wgpu` backend for silky-smooth GPU-accelerated rendering.
- Headless / Low-Resource fallback: `tiny-skia` software rasterization (< 25 MB RAM, operates in VMs or remote desktops without GPU drivers).

---

## 3. Deliverables in `crates/poom-ui`

- `app.rs`: Main Iced `Application` / `Element` state loop.
- `state.rs`: Application state definitions and message enums.
- `theme.rs`: Dark-mode curated color palettes, typography, and styling tokens.
- `widgets/`
  - `waterfall.rs`: Custom canvas program rendering hierarchical Gantt charts.
  - `trace_table.rs`: Virtualized trace list.
  - `inspector.rs`: Span metadata and I/O display panels.
- `subscription.rs`: Real-time event subscription bridge.

---

## 4. Verification & Acceptance Criteria
- [ ] Maintains stable 60 FPS while scrolling a trace list with 10,000 items.
- [ ] Renders a 1,000-span complex multi-agent waterfall canvas in under 16ms frame time.
- [ ] Instantaneous hit-testing (< 1ms) on canvas clicks to populate the inspector panel.
- [ ] Responsive split-pane layout adapting smoothly to window resizing.
