"""
Poom Observability Studio — Live FastAPI Instrumentation Demo
============================================================
This application demonstrates real-time zero-friction telemetry with Poom:
- Pure-Rust IPC transport sending spans directly to ~/.poom/data.redb
- PoomMiddleware automatically tracing HTTP request boundaries and status codes
- Deep nested AI Agent hierarchy: Agent -> Tools -> Retriever -> LLM
- Token metric tracking & cost calculation
- Evaluator scoring and automated error capture with stack traces

Run directly with:
    python main.py
or
    uvicorn main:app --host 127.0.0.1 --port 8000 --reload
"""

import asyncio
import os
import sys
import time
from pathlib import Path
from typing import Any, Dict, List, Optional
from pydantic import BaseModel

# 1. Ensure local poom SDK package is importable
POOM_SDK_DIR = Path(__file__).resolve().parent.parent / "poom"
if str(POOM_SDK_DIR) not in sys.path:
    sys.path.insert(0, str(POOM_SDK_DIR))

import poom
from poom import PoomMiddleware, SpanKind, SpanStatus, trace, trace_tool, set_attribute, set_tokens

from fastapi import FastAPI, HTTPException, Query
from fastapi.responses import JSONResponse

# 2. Initialize Poom native client (auto-connects to daemon socket at /run/user/1000/poom/poom.sock)
poom.init()

app = FastAPI(
    title="Poom Observability Live Demo",
    description="Live FastAPI AI Agent application instrumented with Poom",
    version="1.0.0",
)

# 3. Add ASGI Middleware to trace incoming HTTP requests automatically
app.add_middleware(PoomMiddleware, app_name="poom_fastapi_service")


# -----------------------------------------------------------------------------
# Request & Response Models
# -----------------------------------------------------------------------------

class ChatRequest(BaseModel):
    prompt: str = "Analyze recent revenue metrics and forecast next quarter"
    model: str = "gpt-4o"
    temperature: float = 0.7

class ChatResponse(BaseModel):
    trace_id: Optional[str]
    query: str
    response: str
    tokens_used: int
    duration_ms: float


# Configurable simulation latencies (set DB_LATENCY=0 and LLM_LATENCY=0 for maximum benchmark throughput)
DB_LATENCY = float(os.getenv("DB_LATENCY", "0.002"))  # 2ms simulated vector DB search
LLM_LATENCY = float(os.getenv("LLM_LATENCY", "0.005"))  # 5ms simulated LLM inference

# -----------------------------------------------------------------------------
# Instrumenting Sub-steps: Tools, Retrievers, and LLMs
# -----------------------------------------------------------------------------

@trace_tool(name="execute_vector_search")
async def retrieve_knowledge_chunks(query: str, top_k: int = 3) -> List[Dict[str, Any]]:
    """Simulates a high-speed vector database similarity search."""
    if DB_LATENCY > 0:
        await asyncio.sleep(DB_LATENCY)
    return [
        {"doc_id": "rev-2024-q3", "score": 0.94, "content": "Q3 Enterprise ARR grew 34% YoY to $42.5M."},
        {"doc_id": "rev-2024-q2", "score": 0.88, "content": "Q2 Gross Margin expanded 210 bps to 78.4%."},
        {"doc_id": "guidance-q4", "score": 0.82, "content": "Q4 Projected target range: $48M - $51M ARR."},
    ]


@trace_tool(name="run_financial_calculator")
def compute_growth_rate(prior: float, current: float) -> Dict[str, Any]:
    """Simulates a precision financial calculation tool."""
    rate = ((current - prior) / prior) * 100.0
    return {
        "prior_arr": prior,
        "current_arr": current,
        "delta_percent": round(rate, 2),
        "is_accelerating": rate > 25.0,
    }


@trace(name="openai_completion", kind=SpanKind.LLM)
async def simulate_llm_inference(
    prompt: str,
    context_chunks: List[Dict[str, Any]],
    model: str = "gpt-4o",
) -> str:
    """Simulates an LLM call with realistic token usage and latency."""
    if LLM_LATENCY > 0:
        await asyncio.sleep(LLM_LATENCY)

    # Record model telemetry and parameters into current span attributes
    set_attribute("llm.model", model)
    set_attribute("llm.provider", "openai")
    set_attribute("llm.temperature", 0.7)
    set_attribute("llm.prompt", prompt)
    set_attribute("llm.context_documents", len(context_chunks))

    # Inject token metrics for automated cost estimation in Poom Studio
    prompt_tokens = 540
    completion_tokens = 168
    set_tokens(
        input_tokens=prompt_tokens,
        output_tokens=completion_tokens,
        cached_tokens=128,
    )

    return (
        f"Based on the ARR growth from $31.7M to $42.5M (+34% YoY), Q4 forecast remains strongly on track "
        f"to achieve the guided range of $48M - $51M with expanding gross margins at 78.4%."
    )


@trace(name="orchestrate_agent_workflow", kind=SpanKind.AGENT)
async def run_analyst_agent(user_query: str, model: str = "gpt-4o") -> Dict[str, Any]:
    """Primary multi-step agent reasoning workflow."""
    start_time = time.perf_counter()

    set_attribute("agent.strategy", "ReAct_MultiHop")
    set_attribute("agent.user_query", user_query)

    # 1. Retrieve domain knowledge
    chunks = await retrieve_knowledge_chunks(user_query, top_k=3)

    # 2. Run computation tool
    math_res = compute_growth_rate(prior=31.7, current=42.5)

    # 3. Call LLM with retrieved context
    synthesis = await simulate_llm_inference(user_query, chunks, model=model)

    duration = (time.perf_counter() - start_time) * 1000.0

    # 4. Record an evaluation metric for this trace
    current_trace_id = poom.get_current_trace_id()
    if current_trace_id:
        poom.evaluate(
            trace_id=current_trace_id,
            name="factual_relevance",
            score=0.96,
            comment="Financial figures strictly match verified Q3 ARR data.",
        )

    return {
        "query": user_query,
        "response": synthesis,
        "math_metrics": math_res,
        "duration_ms": round(duration, 2),
        "total_tokens": 540 + 168,
    }


# -----------------------------------------------------------------------------
# FastAPI HTTP Endpoints
# -----------------------------------------------------------------------------

@app.get("/")
async def root():
    """Service status and quick links."""
    return {
        "status": "online",
        "service": "Poom Observability FastAPI Studio Demo",
        "studio_port": 8000,
        "documentation": "Open http://127.0.0.1:8000/docs for Swagger UI",
        "test_endpoints": {
            "POST /api/chat": "Run a multi-step AI Agent trace (Agent -> Tool -> LLM)",
            "POST /api/batch-demo": "Fire 3 diverse agent traces simultaneously to watch the studio live",
            "GET /api/error-demo": "Simulate an unhandled exception to see error tracking & stack traces in studio",
        },
    }


@app.post("/api/chat", response_model=ChatResponse)
async def chat_endpoint(req: ChatRequest):
    """
    Executes a realistic AI Agent trace.
    Every request creates an HTTP root span, correlates nested Tool, Retriever,
    and LLM child spans, and streams them live to Poom Observability Studio.
    """
    res = await run_analyst_agent(user_query=req.prompt, model=req.model)
    return ChatResponse(
        trace_id=poom.get_current_trace_id(),
        query=req.prompt,
        response=res["response"],
        tokens_used=res["total_tokens"],
        duration_ms=res["duration_ms"],
    )


@app.post("/api/batch-demo")
async def batch_demo_endpoint():
    """Fires 3 concurrent agent traces to showcase real-time waterfall streaming."""
    prompts = [
        "Audit Q3 marketing CAC and payback periods",
        "Summarize customer churn reasons in enterprise tier",
        "Generate quarterly forecasting executive briefing",
    ]

    tasks = [run_analyst_agent(p) for p in prompts]
    results = await asyncio.gather(*tasks)

    # Flush spans over IPC so they are immediately available
    poom.flush()

    return {
        "message": "Fired 3 diverse execution traces to Poom Studio successfully!",
        "traces_completed": len(results),
        "details": results,
    }


@app.get("/api/error-demo")
async def error_demo_endpoint(simulate: bool = Query(True, description="Trigger error")):
    """
    Demonstrates Poom's automated error capture.
    Unhandled exceptions in child spans or HTTP endpoints automatically record
    the error type, error message, and backtrace into redb for the inspector.
    """
    @trace(name="faulty_external_api_call", kind=SpanKind.HTTP)
    def call_remote_api():
        set_attribute("http.url", "https://api.external-vendor.com/v1/pricing")
        if simulate:
            raise ConnectionResetError("Remote API gateway closed connection unexpectedly (HTTP 502)")
        return {"status": "ok"}

    try:
        call_remote_api()
    except Exception as e:
        return JSONResponse(
            status_code=502,
            content={
                "error": type(e).__name__,
                "detail": str(e),
                "hint": "Check the waterfall canvas in Poom Studio: this span is highlighted in Red with full backtrace!",
            },
        )

    return {"message": "Success without errors"}


# -----------------------------------------------------------------------------
# Standalone Runner
# -----------------------------------------------------------------------------

if __name__ == "__main__":
    import uvicorn
    print("\n" + "=" * 60)
    print("🚀 Starting Poom FastAPI Demo Service on http://127.0.0.1:8000")
    print("⚡ Poom Observability Studio is configured to trace port 8000 natively")
    print("👉 Open the Studio with: cargo run -p poom-ui")
    print("=" * 60 + "\n")
    uvicorn.run("main:app", host="127.0.0.1", port=8000, reload=True)
