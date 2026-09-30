# Phase 6: Iced Desktop Interface (The Observability Studio)
## Detailed Architectural & Mechanical Specification

---

## 1. Scope, Purpose & Architectural Philosophy

Phase 6 implements the native desktop user interface for Poom: **The Observability Studio**. The studio provides local-first, zero-latency visual analysis for complex LLM architectures, multi-agent reasoning chains, tool invocations, and ASGI pipeline traces.

### 1.1 The Web/Electron Bloat Problem
Traditional LLM observability solutions (e.g., Langfuse, Phoenix, LangSmith) rely heavily on web-based single-page applications (Next.js, React, Node.js) paired with multi-container Docker environments (PostgreSQL, ClickHouse, Redis). This creates severe operational drawbacks for local developers:
1. **Excessive Memory Overhead:** Electron or browser-based dashboards consume 400 MB to 1.5 GB of RAM merely sitting idle.
2. **IPC & Serialization Latency:** Rendering a trace tree with 1,000 spans requires serializing relational database rows into JSON over HTTP, transmitting across loopback, and parsing inside V8, causing noticeable UI freeze and multi-second page loads.
3. **DOM Stutter on Complex Waterfalls:** High-frequency 60 FPS timeline panning across thousands of SVG/DOM nodes in a browser engine strains CPU renderers.

### 1.2 The Poom Pure-Rust Solution
Poom eliminates the web runtime entirely by pairing a native desktop UI written in **Iced** directly with the embedded **`redb`** storage engine:
- **Zero HTTP / Zero JSON Overhead:** The desktop studio queries the local B-Tree storage in-process through zero-copy read transactions (`StorageReader`), decoding compact binary Postcard records in microseconds.
- **Ultra-Lean Memory Footprint:** Built on Iced with `wgpu` hardware acceleration (and `tiny-skia` software fallback), the studio operates in **< 50 MB RAM** even with 10,000 active traces loaded.
- **Hardware-Accelerated 2D Waterfall:** Renders hierarchical multi-agent execution trees using immediate-mode 2D vector primitives (`iced::widget::canvas::Program`) maintaining a strict **60 FPS** frame budget.
- **Live Event Ingestion:** Subscribes to the internal Tokio broadcast channel of the ingestion daemon for instant, zero-lag timeline reactivity as spans arrive from Python workers.

---

## 2. Desktop Application Architecture & The Elm Architecture (TEA)

The desktop application is engineered around **The Elm Architecture (TEA)** natively implemented by Iced, ensuring unidirectional data flow, total memory safety, and zero race conditions.

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                   Poom Desktop Studio                                  │
│                                                                                        │
│   ┌────────────────────────────────────────────────────────────────────────────────┐   │
│   │ Top Bar: Search Query | Time Presets (15m..7d) | Status Filter | Stats Banner  │   │
│   └────────────────────────────────────────────────────────────────────────────────┘   │
│   ┌────────────────────────────────────────┬───────────────────────────────────────┐   │
│   │ Left Pane: Virtualized Trace Table     │ Right Upper: 2D Gantt Waterfall Canvas│   │
│   │ ────────────────────────────────────── │ ───────────────────────────────────── │   │
│   │ [Status] [Root Name] [Dur] [Tokens]    │ Chronological visual timeline bars    │   │
│   │  ✓ Agent Run       1.42s  1.2k tokens  │ Hierarchical depth indents            │   │
│   │  ✗ Tool Extraction  45ms    -- tokens  │ Zoom, pan, and hover tooltips         │   │
│   │  ✓ HTTP GET /query 210ms    -- tokens  ├───────────────────────────────────────┤   │
│   │                                        │ Right Lower: Multi-Tab Span Inspector │   │
│   │                                        │ Overview | I/O Prompts | Metadata     │   │
│   └────────────────────────────────────────┴───────────────────────────────────────┘   │
│   ┌────────────────────────────────────────────────────────────────────────────────┐   │
│   │ Bottom Status Bar: Ingestion Status | Event Counter | RSS Memory | DB Size     │   │
│   └────────────────────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 The Unidirectional TEA Loop
1. **Model (Application State):**
   Holds the immutable ground-truth state of the application, including the connection to storage, search and time filters, loaded trace summaries, the currently selected trace hierarchy, the active span selection, canvas pan/zoom coordinates, and inspector active tabs.
2. **Messages (Discrete Events):**
   An exhaustive enum representing all user inputs (clicks, key presses, mouse wheel deltas, window resizes) and asynchronous background events (database query results, live span arrivals from the ingestion stream).
3. **Update (State Transitions):**
   A pure state transition function taking `(&mut Model, Message)` and producing an optional asynchronous `Task` (Iced command). All expensive operations (querying `redb`, reconstructing trees) are spawned onto background asynchronous tasks to keep the UI thread completely unblocked.
4. **View (Declarative UI Tree):**
   Transforms the current model into a declarative widget tree. Iced computes layout geometries and dispatches drawing commands to the rendering backend.
5. **Subscriptions (Continuous Background Streams):**
   Listens to external asynchronous event sources, notably the live Tokio broadcast channel emitting spans from the ingestion pipeline and window resize events.

---

## 3. Comprehensive State & Message Taxonomy

### 3.1 Application State Hierarchy (`Model`)

The root application state is partitioned into clean, cohesive sub-states:

- **Storage & Infrastructure State:**
  - Database access handle (`Arc<StorageEngine>`).
  - Active daemon connection status (Connected, Offline, Headless).
  - Telemetry counters (total spans received during session, dropped events).

- **Filter & Search State:**
  - Free-text query string (matches span names, trace IDs, model names, user tags).
  - Selected time window preset: `Last15Minutes`, `Last1Hour`, `Last6Hours`, `Last24Hours`, `Last7Days`, `AllTime`, or `Custom(start_nanos, end_nanos)`.
  - Span status filter: `All`, `OnlyErrors`, `OnlySuccess`.
  - Span kind filter: Bitflags or selection of `Agent`, `Chain`, `Llm`, `Tool`, `Function`, `Http`.
  - Live auto-scroll toggle: Whether incoming traces automatically shift the view to the latest event.

- **Trace Explorer State:**
  - Cached list of trace summary headers for the active filter window.
  - Total trace count matching filter.
  - Current pagination / scroll offset.
  - Active trace selection (`Option<TraceId>`).
  - Loading indicator state (Idle, FetchingTraces, ErrorLoading).

- **Trace Detail & Waterfall Canvas State:**
  - Active trace tree root node (`Option<SpanNode>`).
  - Flattened chronological span layout list computed from the active tree.
  - Trace time bounds: absolute start timestamp (nanoseconds), total trace duration (nanoseconds).
  - Timeline horizontal pan offset (in screen pixels).
  - Timeline horizontal zoom scale (scalar multiplier, default 1.0, range 0.05 to 50.0).
  - Timeline vertical scroll offset.
  - Hovered span identifier (`Option<SpanId>`) and hovered screen coordinates for tooltip positioning.
  - Selected span identifier (`Option<SpanId>`).

- **Span Inspector State:**
  - Full record of the selected span (`Option<SpanRecord>`).
  - Active tab: `Overview`, `PromptsAndCompletions`, `Attributes`, `EventsTimeline`, `Evaluations`.
  - JSON formatting / Markdown rendering toggle for prompt bodies.
  - Search filter within attribute keys.
  - Clipboard status (feedback indicator when copying IDs or prompt text).

- **Layout & Window State:**
  - Main horizontal split ratio (Trace table width vs Detail canvas width, default 35% / 65%).
  - Right pane vertical split ratio (Waterfall canvas height vs Inspector height, default 55% / 45%).
  - Window dimensions (width, height) and system scale factor.

---

### 3.2 Message (Event) Taxonomy

The message enum captures every interaction and asynchronous trigger:

#### Navigation & Selection Messages
- `SelectTrace(TraceId)`: User clicks a trace in the left table; triggers an asynchronous background query to fetch the full trace tree.
- `SelectSpan(SpanId)`: User clicks a span bar in the 2D waterfall canvas or an entry in a breadcrumb; populates the inspector.
- `HoverSpan(Option<SpanId>, Option<Point>)`: Mouse pointer moves over or away from a span bar in the canvas; drives tooltip display.
- `DeselectAll`: Clears active span selection.

#### Filter & Query Messages
- `SearchQueryChanged(String)`: User types into the search box; triggers debounced query reload.
- `TimeWindowChanged(TimeWindowPreset)`: User changes the time window filter.
- `StatusFilterChanged(StatusFilter)`: User selects All / Success / Error filter.
- `KindFilterToggled(SpanKind)`: User toggles visibility of specific span categories.
- `ToggleAutoScroll(bool)`: Enables or disables pinning to the newest live traces.
- `RefreshTraces`: Manually forces a storage scan for the current filter criteria.

#### Asynchronous Data Responses (Worker Task Callbacks)
- `TracesLoaded(Result<Vec<TraceSummary>, StorageError>)`: Background reader finishes scanning recent traces; updates trace explorer list.
- `TraceTreeLoaded(Result<Option<SpanNode>, StorageError>)`: Background reader completes trace span fetching and tree assembly; re-computes waterfall geometry.
- `EvaluationsLoaded(Result<Vec<EvaluationRecord>, StorageError>)`: Associated evaluations fetched for the selected trace.

#### Live Event Stream Messages
- `LiveSpanReceived(SpanRecord)`: Real-time span received over the daemon broadcast subscription; updates counters and prepends to trace table if matching filters.
- `LiveSubscriptionLagged(u64)`: Broadcast channel dropped messages due to UI lag; logs warning banner.

#### Waterfall Canvas Interaction Messages
- `CanvasScrolled(f32, f32)`: Mouse wheel or trackpad scroll delta applied to canvas horizontal/vertical offsets.
- `CanvasZoomed(f32, Point)`: Pinch-to-zoom or Ctrl+Wheel event scaling the timeline around the cursor point.
- `CanvasDragStarted(Point)`: User presses left mouse button on canvas background to initiate panning.
- `CanvasDragged(Vector)`: Mouse moves during drag; updates pan coordinates.
- `CanvasDragEnded`: Mouse button released.
- `ResetCanvasView`: Resets zoom to 1.0 and centers the entire trace duration within the visible viewport.

#### Inspector & Utility Messages
- `InspectorTabSelected(InspectorTab)`: Switches between Overview, Prompts, Attributes, Events, and Evaluations.
- `CopyToClipboard(String)`: Copies trace ID, span ID, prompt text, or JSON to the system clipboard.
- `HorizontalSplitResized(f32)`: User drags the left/right pane divider.
- `VerticalSplitResized(f32)`: User drags the waterfall/inspector divider.
- `WindowResized(u32, u32)`: Window size changed; triggers canvas layout recalculation.

---

## 4. Subsystem Deep Dive & Component Specifications

### 4.1 Subsystem 1: Live Trace Explorer (Left Panel)

The Trace Explorer provides a high-density, real-time list of root traces.

#### Architectural Mechanics:
1. **Trace Summarization:**
   Instead of loading thousands of full `SpanRecord` structs with complete attributes into memory, the reader queries `TIME_INDEX` to find trace IDs, reads root spans, and maps them to lightweight `TraceSummary` structs:
   - `trace_id`: 16-byte UUIDv7.
   - `root_name`: Compact string (e.g., "AgentWorkflow", "POST /api/chat").
   - `root_kind`: `SpanKind` (Agent, Chain, LLM, etc.).
   - `start_time_unix_nanos`: 64-bit integer timestamp.
   - `duration_nanos`: Optional 64-bit duration.
   - `status`: `SpanStatus` (Ok or Error).
   - `span_count`: Total spans participating in the trace.
   - `total_tokens`: Aggregated token count across all descendant spans.
   - `total_cost_usd`: Aggregated financial cost across all descendant spans.

2. **Virtualized List Rendering:**
   To guarantee smooth 60 FPS scrolling when tens of thousands of traces exist:
   - The widget calculates the visible row window based on the viewport height and a fixed item row height (e.g., 52 pixels).
   - Only items in the visible slice plus an overscan buffer (5 items above, 5 items below) are instantiated into Iced widgets.
   - Off-screen items consume zero widget layout overhead.

3. **Row Visual Anatomy:**
   Each row contains:
   - **Status Indicator Pill:** A circular accent badge (Emerald Green for `Ok`, Crimson Red with exclamation icon for `Error`).
   - **Root Name & Kind:** Bold title with a color-coded tag showing `Agent`, `Tool`, `LLM`, or `HTTP`.
   - **Relative Timestamp:** Humanized relative time ("just now", "12s ago", "4m ago", "Yesterday").
   - **Duration Badge:** Color-scaled based on latency thresholds (Green: < 250ms, Amber: 250ms–1.5s, Rose: > 1.5s).
   - **Token & Cost Counter:** Displayed as subtle secondary text (e.g., "1.4k tokens · $0.021").
   - **Selection State:** Active item highlighted with a slate-800 background and left accent bar.

---

### 4.2 Subsystem 2: 2D Gantt Waterfall Canvas (`canvas::Program`)

The waterfall canvas is the visual centerpiece of Poom. It transforms hierarchical parent-child execution spans into a 2D timeline Gantt chart.

#### 4.2.1 Coordinate Mathematics & Projection Model

Let the selected trace execution span tree have:
- Trace start time: $T_{start} = \min_{s \in \text{spans}} (\text{start\_time\_unix\_nanos}_s)$
- Trace end time: $T_{end} = \max_{s \in \text{spans}} (\text{end\_time\_unix\_nanos}_s)$
- Total trace duration: $\Delta T = \max(T_{end} - T_{start}, 1)$

For a given span $s$:
- Relative start time: $\delta t_{start} = s.\text{start\_time\_unix\_nanos} - T_{start}$
- Span duration: $\delta t_{dur} = \max(s.\text{duration\_nanos}(), 1)$

Given viewport parameters:
- $W_{canvas}$: Width of canvas drawable area in pixels.
- $X_{pad}$: Left padding for tree branch labels (default: 220 pixels).
- $W_{timeline} = W_{canvas} - X_{pad}$: Usable timeline width.
- $Z$: Horizontal zoom factor ($Z \ge 1.0$).
- $O_x$: Horizontal pan offset in pixels.
- $O_y$: Vertical pan offset in pixels.
- $H_{row}$: Fixed row height per span (e.g., 32 pixels).
- $H_{header}$: Height of the timeline tick-mark ruler (e.g., 28 pixels).

The visual geometry for span $s$ at tree-flattened row index $i$ is calculated as:
$$\text{Bar } X = X_{pad} + \left( \frac{\delta t_{start}}{\Delta T} \times W_{timeline} \times Z \right) + O_x$$
$$\text{Bar Width} = \max\left( \frac{\delta t_{dur}}{\Delta T} \times W_{timeline} \times Z, \text{MIN\_WIDTH} \right)$$
$$\text{Bar } Y = H_{header} + (i \times H_{row}) + O_y$$
$$\text{Bar Height} = H_{row} - 6 \quad (\text{providing 3px vertical padding top and bottom})$$

Where $\text{MIN\_WIDTH} = 4.0\text{px}$ guarantees that instantaneous events (e.g., 100μs tool calls) remain visible and clickable even at wide zoom levels.

#### 4.2.2 Visual Encoding & Span Kind Color Tokens

To allow instant cognitive parsing of complex agent reasoning chains, each `SpanKind` has an exclusive, high-contrast color token:

| SpanKind | Semantic Role | Fill Color (Dark Theme) | Border Accent | Text Display |
|---|---|---|---|---|
| **`Agent`** | High-level orchestration & reasoning loop | Deep Purple (`#8B5CF6`) | `#A78BFA` | Purple Badge |
| **`Chain`** | Sequential DAG pipeline / retrieval | Slate Blue (`#3B82F6`) | `#60A5FA` | Blue Badge |
| **`Llm`** | Model inference (tokens, prompts) | Emerald Green (`#10B981`) | `#34D399` | Green Badge |
| **`Tool`** | External API / function invocation | Amber / Gold (`#F59E0B`) | `#FBBF24` | Amber Badge |
| **`Function`**| Internal application logic / helper | Neutral Slate (`#64748B`) | `#94A3B8` | Slate Badge |
| **`Http`** | ASGI / Web network boundary | Sky Cyan (`#06B6D4`) | `#38BDF8` | Cyan Badge |

#### 4.2.3 Tree Hierarchy & Connector Rendering
To the left of the timeline bar ($0$ to $X_{pad}$):
- Each span renders its name preceded by hierarchical indentation ($16\text{px} \times \text{depth}$).
- Subtle vertical and horizontal connector lines (1px width, color `#334155`) connect child spans to their parent row, making deep recursive tool calls and agent loops instantly understandable.
- If a span produced an error (`SpanStatus::Error`), a bright crimson dot or warning glyph is rendered next to the name.

#### 4.2.4 Time Tick Ruler & Background Grid
At the top of the canvas ($0$ to $H_{header}$):
- The ruler dynamically computes human-readable time intervals based on zoom factor $Z$ (e.g., 50ms, 100ms, 250ms, 500ms, 1s, 5s).
- Renders subtle dashed vertical gridlines spanning the entire height of the canvas at each interval.
- Displays tick labels in monospace font (e.g., "+250ms", "+500ms").

#### 4.2.5 Hit-Testing, Interaction & Tooltips
- **Hover Detection:**
  On every `canvas::Event::Mouse(mouse::Event::CursorMoved { position })`, the canvas program computes the row index:
  $$i = \left\lfloor \frac{\text{position.y} - H_{header} - O_y}{H_{row}} \right\rfloor$$
  If $i$ is within bounds, it checks whether $\text{position.x} \ge \text{Bar } X$ and $\text{position.x} \le \text{Bar } X + \text{Bar Width}$.
  If matched, `HoverSpan(Some(span_id), Some(position))` is dispatched.
- **Rich Hover Tooltip:**
  When a span is hovered, an overlay card renders showing:
  - Full span name and kind badge.
  - Duration with microsecond precision (e.g., "142.8 ms").
  - Self time vs total child execution time.
  - Model name and token breakdown (if LLM span).
  - Status outcome (Ok / Error description).
- **Click Selection:**
  Clicking a bar dispatches `SelectSpan(span_id)`, outlining the bar with a 2px white highlight border and populating the Span Inspector.
- **Pan & Zoom:**
  - Mouse wheel vertical scroll pans up/down ($O_y$).
  - Shift + Mouse wheel pans left/right ($O_x$).
  - Ctrl + Mouse wheel zooms horizontally ($Z$) anchored around the mouse cursor position so the timeline expands naturally where the user points.

---

### 4.3 Subsystem 3: Multi-Tab Span Inspector (Right Lower Panel)

When a span is selected, the Inspector panel exposes all forensic execution details across structured tabs.

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ [Overview]  [Prompts & Completions]  [Attributes (14)]  [Events (3)]  [Evaluations (1)] │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ Model: gpt-4o-2024-08-06          Tokens: 840 input + 162 output = 1,002 total         │
│ Status: Ok                        Duration: 1.28s (Start: 22:54:10.120)                │
│ Estimated Cost: $0.0042 USD       Parent: AgentWorkflow [7f3b...a12]                   │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ System Prompt:                                                                         │
│ "You are an expert financial analyst. Analyze the quarterly earnings table below..."   │
│                                                                                        │
│ User Prompt:                                                                           │
│ "Summarize Q3 revenue growth and compare it with operating margins."                   │
│                                                                                        │
│ Assistant Output:                                                                      │
│ "Based on the Q3 report, revenue grew 14.2% year-over-year, while operating margin..." │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Tab 1: Overview
- **Executive Card:** Span kind, parent span breadcrumb link, execution timestamps (UTC and local), duration, and status.
- **Error Diagnostic Panel (if `SpanStatus::Error`):**
  - Highlighted crimson error container.
  - Error type (e.g., `openai.RateLimitError`, `KeyError`, `TimeoutException`).
  - Human-readable error message.
  - Formatted stack trace with monospace syntax highlighting and a "Copy Stack Trace" button.
- **LLM Metrics Summary:**
  - Token breakdown bar chart: Prompt Tokens, Completion Tokens, Cached Tokens, Reasoning Tokens.
  - Financial cost computed to 4 decimal places in USD.

#### Tab 2: Prompts & Completions (Inputs & Outputs)
- Tailored specifically for `SpanKind::Llm` and `SpanKind::Tool`:
  - **Inputs:** Extracts attributes like `prompt`, `messages`, `system_prompt`, `input`, `args`.
  - **Outputs:** Extracts `completion`, `response`, `content`, `output`, `result`.
- **View Options:**
  - Formatted Markdown with syntax highlighting for model markdown output.
  - Raw JSON view with tree expansion.
  - "Copy to Clipboard" button for quick prompt extraction.

#### Tab 3: Attributes & Metadata
- Renders an alphabetized, searchable table of all `SpanRecord.attributes`.
- Handles rich `AttributeValue` variants:
  - Strings, Booleans, Integers, Floats rendered with distinct typography.
  - Nested Arrays and Maps rendered with recursive collapsible indentations.
- Attribute search bar filtering keys in real time.

#### Tab 4: Events Timeline
- Renders point-in-time `EventRecord` items chronologically:
  - Offset timestamp relative to span start (e.g., "+12ms: First token arrived", "+450ms: Tool call dispatched").
  - Event name and custom key-value attributes.

#### Tab 5: Evaluations & Feedback
- Displays all `EvaluationRecord` items linked to this span or trace:
  - Evaluation metric name (e.g., "hallucination_score", "relevance", "user_thumbs_up").
  - Value rendered as a score badge, boolean pill, or categorical label.
  - Reviewer comment or automated evaluator explanation.

---

### 4.4 Subsystem 4: Design System, Theming & Typography

The UI adheres to a sleek, modern, professional dark-mode design system tailored for high-focus developer observability.

#### 4.4.1 Color System Tokens

```
Background Primary:      #0B0F19  (Deep space slate)
Background Surface:      #111827  (Tailwind Slate-900)
Background Card / Row:   #1E293B  (Slate-800)
Background Hover:        #334155  (Slate-700)
Border Subtle:           #1F2937  (Slate-800)
Border Focused / Active: #475569  (Slate-600)

Text Primary:            #F8FAFC  (Slate-50, 95% opacity)
Text Secondary:          #94A3B8  (Slate-400)
Text Muted / Disabled:   #64748B  (Slate-500)

Accent Success:          #10B981  (Emerald-500)
Accent Error / Danger:   #EF4444  (Red-500)
Accent Warning:          #F59E0B  (Amber-500)
Accent Info:             #3B82F6  (Blue-500)
```

#### 4.4.2 Typography & Fonts
- **Interface Font:** System sans-serif or embedded Inter font for UI labels, titles, and table text.
- **Monospace Font:** JetBrains Mono or system monospace for trace IDs, timestamps, token numbers, JSON inspectors, and error backtraces.
- **Font Scaling:** Native High-DPI support adapting automatically to OS scaling factors (100%, 125%, 150%, 200%).

---

### 4.5 Subsystem 5: Storage Integration & Non-Blocking Background Queries

The UI runs on a dedicated rendering thread and must **never** block on disk I/O or B-Tree locks.

#### 4.5.1 Query Workflow via `iced::Task`
All database operations use asynchronous background tasks:
1. When a user changes filters or selects a trace, the `update()` function constructs an `iced::Task::perform(...)`.
2. The task runs on Tokio's multi-thread runtime pool, opening a short-lived read transaction (`StorageReader::new(&db)`).
3. The reader performs B-Tree range scans, fetches spans, sorts children chronologically, and returns the result across an asynchronous channel.
4. The result arrives as a `TracesLoaded` or `TraceTreeLoaded` message on the main thread, updating the Model and scheduling a redrawing frame.
5. Even if a trace spans 10,000 nodes, the UI thread remains completely responsive at 60 FPS while the background worker deserializes Postcard blobs.

---

### 4.6 Subsystem 6: Real-Time Live Streaming & Ingestion Subscription

When traces are actively streaming from FastAPI or Python scripts, the UI displays them dynamically.

#### 4.6.1 Subscription Architecture
- The application implements an `iced::subscription::Subscription`:
  - Listens to the daemon's internal `EventBroadcaster` (`tokio::sync::broadcast::Receiver<SpanRecord>`).
  - Converts incoming broadcast events into `Message::LiveSpanReceived(span)`.
- **Frame Rate Throttling & Coalescing:**
  - If Python workers stream 50,000 spans/sec, re-rendering on every single span would saturate the UI event loop.
  - The subscription buffers spans into a thread-safe staging queue and emits a coalesced batch message at a fixed UI cadence (30 Hz to 60 Hz).
  - This guarantees silky-smooth UI reactivity without queue starvation or CPU spikes.

---

### 4.7 Subsystem 7: Dual Rendering Backend (`wgpu` vs `tiny-skia`)

The application supports dual backend compilation configurations:
1. **`wgpu` Backend (Default):**
   - Direct hardware acceleration via Vulkan (Linux), Metal (macOS), or DirectX 12 (Windows).
   - High-speed canvas rasterization and smooth 144Hz+ monitor support.
2. **`tiny-skia` Backend (Fallback / Headless Environments):**
   - Pure software CPU rasterization.
   - Operates flawlessly inside virtual machines, cloud devboxes, Docker containers with X11/Wayland forwarding, or systems without GPU drivers.
   - Keeps base memory footprint below **25 MB RSS**.

---

## 5. Crate Topology & Deliverables in `crates/poom-ui`

The Phase 6 deliverables are isolated in `crates/poom-ui`:

```
crates/poom-ui/
├── Cargo.toml                     # Iced dependencies, features, and styling
└── src/
    ├── lib.rs                     # Library root, public launcher functions
    ├── app.rs                     # Master Iced Application entrypoint, update, and view
    ├── state.rs                   # Complete Model state structs and Message enum
    ├── error.rs                   # UI error types and handling
    ├── subscription.rs            # Live broadcast stream adapter
    │
    ├── theme/                     # Design system & tokens
    │   ├── mod.rs                 # Theme exports
    │   ├── colors.rs              # Curated dark-mode color palette constants
    │   ├── style.rs               # Custom Iced widget styles (buttons, containers, scrolls)
    │   └── typography.rs          # Font definitions and sizing
    │
    ├── views/                     # High-level screen regions
    │   ├── top_bar.rs             # Search input, time window selector, stats badges
    │   ├── status_bar.rs          # Ingestion counters, DB metrics, memory display
    │   ├── layout.rs              # Split-pane container and responsive grid
    │   └── empty_state.rs         # Clean placeholder visuals when no traces exist
    │
    └── widgets/                   # Specialized interactive UI components
        ├── trace_table.rs         # Virtualized live trace list with sorting
        │
        ├── waterfall/             # 2D Gantt Canvas Program
        │   ├── mod.rs             # Waterfall widget module
        │   ├── program.rs         # iced::widget::canvas::Program implementation
        │   ├── layout.rs          # Tree flattening and coordinate projection math
        │   ├── colors.rs          # SpanKind color coding
        │   └── tooltip.rs         # Rich hover floating card
        │
        └── inspector/             # Multi-tab span inspection panel
            ├── mod.rs             # Inspector container
            ├── overview.rs        # Status, timing, and error backtrace view
            ├── prompts.rs         # LLM input/output Markdown and JSON renderer
            ├── attributes.rs      # Canonical key-value metadata table
            ├── events.rs          # Point-in-time span event timeline
            └── evaluations.rs     # Evaluation scores and feedback panel
```

---

## 6. Verification, Benchmarks & Acceptance Criteria

To declare Phase 6 complete and ready for production, the implementation will be validated against strict acceptance criteria:

### 6.1 Performance Benchmarks
- [ ] **Memory Footprint:** Clean startup RSS memory must be **< 35 MB**; memory under 10,000 cached traces must remain **< 65 MB**.
- [ ] **Frame Rate Stability:** Canvas panning and zooming across a 1,000-span trace tree must maintain a stable **60 FPS** without frame drops.
- [ ] **Instant Hit-Testing:** Cursor hover hit-testing on the waterfall canvas must resolve in **< 1.0 millisecond**.
- [ ] **Background Query Latency:** Selecting a trace must reconstruct and render the full hierarchical tree in **< 15 milliseconds** from `redb`.

### 6.2 Ergonomics & Functional Verification
- [ ] **Trace Filtering:** Real-time search query, status filters (Errors only), and time presets filter the visible trace list instantaneously.
- [ ] **Tree Assembly Correctness:** Hierarchical agent execution with multi-nested tools displays exact parent-child branch alignments and duration bars.
- [ ] **Error Visibility:** Any failed span displays clear red visual indicators on both the trace list, the waterfall bar, and renders the complete traceback in the inspector.
- [ ] **Live Ingestion Reactivity:** Streaming spans from a concurrent Python FastAPI application dynamically populate the trace explorer in real time without crashing or freezing the interface.
- [ ] **Zero Unhandled Panics:** Gracefully handles empty databases, missing spans, corrupted records, and window resize events without panicking.
