# Poom (पूम) — Pure-Rust Observability Studio

> **High-performance, local-first, zero-friction LLM & AI agent observability studio built entirely in pure Rust.**
> No Docker. No Postgres. No ClickHouse. No cloud accounts. Zero cognitive overhead.

---

## 1. What is Poom?

**Poom** is an ultra-low-overhead, local-first observability and trace profiling studio designed specifically for developers building AI agents, LLM pipelines, and high-throughput FastAPI services.

Traditional AI observability requires orchestrating massive, enterprise-focused platforms with multiple containers, external databases, cloud credentials, and heavy frontend stacks. Poom flips this model upside down: **a single, native Rust binary with an embedded ACID database and GPU-accelerated desktop canvas**, providing instant observability with single-line Python decorators.

```text
┌─────────────────────────────────────────────────────────────┐
│                    Your Application Code                    │
│      FastAPI  ·  LangChain  ·  LlamaIndex  ·  Custom ReAct  │
└──────────────────────────────┬──────────────────────────────┘
                               │  @poom.trace / PoomMiddleware
                               ▼
┌─────────────────────────────────────────────────────────────┐
│             poom_native (Rust C-Python Bridge)             │
│        Lock-free Crossbeam Ring Buffer (< 50ns hook)        │
└──────────────────────────────┬──────────────────────────────┘
                               │  Postcard Binary IPC (.sock)
                               ▼
┌─────────────────────────────────────────────────────────────┐
│               poom-daemon (Pure-Rust Daemon)                │
│    Tokenization · Pricing Normalizer · redb ACID Engine     │
└──────────────────────────────┬──────────────────────────────┘
                               │  Lock-Free Ring Broadcast
                               ▼
┌─────────────────────────────────────────────────────────────┐
│            poom-ui (Native Iced Desktop Studio)             │
│    60 FPS Gantt Timeline · Dark/Light Mode · 2D Canvas      │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Why Poom Eliminates the Heavy Observability Stack

Modern developers building with LLMs often find that existing observability tools (Langfuse, Arize Phoenix, OpenTelemetry Collector + Jaeger/Prometheus, Datadog) bring unnecessary operational burdens to local development:

| Traditional Observability Stack | The Poom Pure-Rust Architecture |
| :--- | :--- |
| **Requires Docker Daemon & Compose** running Postgres, Redis, ClickHouse, and Node.js. | **Single self-contained native binary** with zero external runtime dependencies. |
| **Consumes 4 GB – 8 GB of RAM** at idle just running containers. | **Consumes < 30 MB of RAM** total across storage and ingestion. |
| **Multi-Tenant Friction**: Forces signup, organization setup, project creation, and API key management. | **Zero friction**: Open the studio and trace immediately. No accounts, projects, or API keys. |
| **Network Bottleneck**: HTTP/1.1 or gRPC over loopback TCP (`http://127.0.0.1:4318`) introduces 2ms–15ms latency per request. | **Sub-millisecond IPC**: Unix Domain Sockets with binary Postcard framing; zero TCP network roundtrips. |
| **Telemetric Dropouts**: When traffic spikes, HTTP collectors queue memory and drop spans. | **Zero-Allocation Ring Buffers**: Lock-free worker thread flushes batches directly to disk. |
| **Heavy Web Frontends**: Electron or complex web apps with noticeable frame drops under large trace trees. | **Native GPU-Accelerated GUI**: Built on Iced with `wgpu` and `tiny-skia` for smooth 60 FPS Gantt waterfall rendering. |

---

## 3. Elite Features Engineered in Pure Rust

### 🦀 1. Pure-Rust Lock-Free IPC Transport (`poom-transport` & `poom-protocol`)
- Native Python C-extension (`poom_native`) built with PyO3.
- Writes telemetry spans into a lock-free Crossbeam ring buffer without holding Python’s Global Interpreter Lock (GIL).
- Spans are serialized with **Postcard**, an ultra-compact, `#[no_std]`-compatible binary serializer that is orders of magnitude smaller and faster than JSON and Protobuf.
- Streams over Unix Domain Sockets (`$XDG_RUNTIME_DIR/poom/poom.sock`), eliminating loopback TCP overhead.

### 💾 2. Embedded ACID Storage Engine (`poom-storage`)
- Powered by **redb**, a 100% Rust embedded transactional key-value store with MVCC architecture and crash-safe ACID semantics.
- Stored directly in `~/.poom/data.redb` with zero database configuration or background processes.
- Automatic secondary indices for fast lookups by `TraceId`, timestamp ranges, root span status, and custom tags.
- Built-in TTL cascading retention engine for automated pruning without database fragmentation.

### ⚡ 3. Live Tokenization & Financial Cost Engine (`poom-daemon`)
- Integrated with `tiktoken-rs` for zero-overhead local BPE tokenization.
- Automatically computes token counts if LLM provider responses omit usage metadata.
- Built-in price normalization engine supporting OpenAI, Anthropic, Google Gemini, and open-weights models.

### 🎨 4. Native GPU-Accelerated 2D Gantt Waterfall Canvas (`poom-ui`)
- Built using **Iced 0.13**, featuring an immediate-mode 2D Canvas with WGPU hardware acceleration.
- Virtualized row rendering displaying thousands of nested spans without frame drops.
- Interactive pan, zoom (0.1x to 50x), depth-first tree indentation, branch connectors, and hover telemetry cards.
- **Span Inspector**: Comprehensive multi-tab view covering Timing, Prompts & I/O, Attributes, Events, and Evaluation metrics.
- **Studio Customization**: Built-in **Dark / Light mode**, dynamic **real-time font scaling** (70% to 160%), and native **FastAPI Port 8000** integration.

---

## 4. Python & FastAPI SDK API Reference

### 4.1 Initialization

Before recording traces, initialize the background worker thread. This creates the IPC bridge to the local daemon:

```python
import poom

# Connect to the local Poom daemon (takes < 1ms)
poom.init_tracer()

# Both `init()` and `init_tracer()` are supported interchangeably:
# poom.init()
```

---

### 4.2 FastAPI Auto-Instrumentation (`PoomMiddleware`)

The easiest way to trace any FastAPI or Starlette application is by adding the ASGI middleware. It creates a root span for every HTTP request and correlates all inner function calls automatically:

```python
from fastapi import FastAPI
import poom
from poom import PoomMiddleware

poom.init_tracer()

app = FastAPI(title="My Service")

# 1-line auto-instrumentation
app.add_middleware(PoomMiddleware, app_name="my_fastapi_service")
```

**What `PoomMiddleware` captures automatically:**
- HTTP Method (`GET`, `POST`, `PUT`, `DELETE`)
- Path & Query Parameters (`/api/chat?model=gpt-4o`)
- HTTP Status Code (`200`, `422`, `500`, `502`)
- Exact nanosecond server latency
- Injects `x-poom-trace-id` into outbound response headers for downstream frontend correlation.

---

### 4.3 Function Decorators (`@poom.trace`)

Use `@trace` to instrument any synchronous or asynchronous Python function. Poom automatically preserves trace hierarchies across concurrent `asyncio` tasks using `contextvars`.

```python
from poom import trace, SpanKind, set_attribute, set_tokens

# 1. Instrument high-level Agent workflows
@trace(name="analyst_agent", kind=SpanKind.AGENT)
async def run_agent(query: str):
    # Any child @trace calls are automatically nested under this agent!
    return await generate_report(query)

# 2. Instrument LLM inference calls
@trace(name="openai_completion", kind=SpanKind.LLM)
async def call_llm(prompt: str, model: str = "gpt-4o"):
    # Inject model metadata into the current span
    set_attribute("llm.model", model)
    set_attribute("llm.temperature", 0.7)
    set_attribute("llm.prompt", prompt)

    response = await my_openai_client(prompt)

    # Record token usage for automatic cost computation in Poom Studio
    set_tokens(input_tokens=420, output_tokens=150, cached_tokens=64)

    return response
```

#### Supported `SpanKind` Constants:
- `SpanKind.AGENT`: Autonomous agent workflows and multi-step reasoning chains (Purple).
- `SpanKind.LLM`: Direct inference calls to OpenAI, Anthropic, Gemini, or local models (Emerald).
- `SpanKind.TOOL`: External tool executions, API lookups, or calculations (Amber).
- `SpanKind.CHAIN`: Orchestration sequences and pipeline steps (Blue).
- `SpanKind.HTTP`: Web service request boundaries (Cyan).
- `SpanKind.FUNCTION`: Internal business logic and utility functions (Slate).

---

### 4.4 Agent Tool Decorator (`@poom.trace_tool`)

Specialized decorator designed for agent function-calling. Automatically captures function input parameters and returned results into span attributes:

```python
from poom import trace_tool

@trace_tool(name="vector_database_search")
async def search_knowledge_base(query: str, top_k: int = 5):
    # Input parameters (`query`, `top_k`) are automatically serialized into span attributes!
    docs = await db.similarity_search(query, k=top_k)
    # Return value is automatically saved to "tool.output"
    return docs
```

---

### 4.5 Context Manager (`with poom.span(...)`)

For ad-hoc execution blocks where wrapping an entire function in a decorator is not ideal:

```python
import poom
from poom import SpanKind

with poom.span("retrieve_customer_profile", kind=SpanKind.TOOL) as s:
    s.set_attribute("user.id", 42)
    profile = fetch_from_redis(user_id=42)
    s.set_attribute("cache_hit", True)
```

Supports both synchronous (`with`) and asynchronous (`async with`) blocks.

---

### 4.6 Trace Evaluation & Quality Scoring (`poom.evaluate`)

Attach automated quality scores, safety ratings, or feedback to traces:

```python
import poom

trace_id = poom.get_current_trace_id()

if trace_id:
    poom.evaluate(
        trace_id=trace_id,
        name="answer_relevance",
        score=0.95,                              # Numeric float (0.0 - 1.0) or bool or string
        comment="Synthesized answer accurately addresses all bullet points.",
    )
```

---

### 4.7 Context Accessors

| Function | Description |
| :--- | :--- |
| `poom.get_current_trace_id()` | Returns the active 128-bit UUIDv7 trace ID string, or `None`. |
| `poom.get_current_parent_span_id()` | Returns the ID of the immediate parent span in the call stack. |
| `poom.set_attribute(key, value)` | Injects key-value metadata into the active span. |
| `poom.set_tokens(input, output, cached)` | Injects token metrics for financial cost calculation. |
| `poom.flush(timeout_seconds=2.0)` | Blocks until all pending IPC spans are flushed to the daemon. |

---

## 5. Complete Production FastAPI Example

Here is a complete, copy-pasteable FastAPI application demonstrating an AI Agent with nested Tools, Retrievers, LLMs, and automated error handling:

```python
import asyncio
from typing import Any, Dict, List
from fastapi import FastAPI
from pydantic import BaseModel

import poom
from poom import PoomMiddleware, SpanKind, trace, trace_tool, set_attribute, set_tokens

# 1. Initialize Poom background native worker
poom.init_tracer()

app = FastAPI(title="Poom AI Agent Service")

# 2. Add ASGI middleware for automatic HTTP tracing
app.add_middleware(PoomMiddleware, app_name="poom_fastapi_service")


class AgentRequest(BaseModel):
    prompt: str = "Analyze Q3 revenue trends"
    model: str = "gpt-4o"


# Tool: Simulated vector database similarity search
@trace_tool(name="execute_vector_search")
async def search_docs(query: str) -> List[Dict[str, Any]]:
    await asyncio.sleep(0.015)
    return [{"id": "doc-1", "content": "Q3 revenue reached $42.5M, up 34% YoY."}]


# LLM: Simulated model inference with token tracking
@trace(name="openai_completion", kind=SpanKind.LLM)
async def generate_response(prompt: str, context: List[Dict[str, Any]], model: str) -> str:
    await asyncio.sleep(0.040)
    set_attribute("llm.model", model)
    set_attribute("llm.prompt", prompt)
    set_tokens(input_tokens=350, output_tokens=120, cached_tokens=50)
    return "Q3 enterprise growth is strong at 34% YoY."


# Agent: Top-level orchestration
@trace(name="orchestrate_agent", kind=SpanKind.AGENT)
async def run_agent(prompt: str, model: str) -> Dict[str, Any]:
    docs = await search_docs(prompt)
    answer = await generate_response(prompt, docs, model)
    
    # Record evaluation score
    trace_id = poom.get_current_trace_id()
    if trace_id:
        poom.evaluate(trace_id=trace_id, name="relevance", score=0.98)
        
    return {"query": prompt, "answer": answer}


@app.post("/api/chat")
async def chat_endpoint(req: AgentRequest):
    result = await run_agent(req.prompt, req.model)
    return {"trace_id": poom.get_current_trace_id(), "result": result}


if __name__ == "__main__":
    import uvicorn
    uvicorn.run("main:app", host="127.0.0.1", port=8000, reload=True)
```

---

## 6. How to Run the Studio & Load Test

### Step 1: Launch Poom Observability Studio
In your terminal, run:
```bash
cargo run -p poom-ui
```
*The native GPU window opens, starting the embedded ingestion daemon listening on `/run/user/1000/poom/poom.sock`.*

### Step 2: Start Your FastAPI App
In a second terminal:
```bash
python3 python/fastapi/main.py
# or: uvicorn main:app --port 8000 --reload
```

### Step 3: Load Test With `wrk`
Send concurrent agent requests using the pre-configured Lua script:
```bash
wrk -t2 -c10 -d10s --latency -s python/fastapi/load_test.lua http://127.0.0.1:8000
```

### What You'll See in the Studio:
1. **Live Ingestion**: The bottom-left counter updates in real time (`Live Events: 1,500+`).
2. **Interactive Waterfall**: Full hierarchical execution trees appear dynamically on the left.
3. **Inspector Panel**: Click any span to inspect exact nanosecond latency, model tokens, financial costs, and stack traces on errors.
4. **Settings Panel**: Click **`⚙ Settings`** at the bottom left to change UI font scaling, toggle Light/Dark mode, or copy integration snippets.
