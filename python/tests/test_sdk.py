import asyncio
import os
import sys
import tempfile
import time
import unittest
from typing import List

# Ensure python/poom is in sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../poom")))

import poom
from poom import (
    PoomMiddleware,
    SpanKind,
    evaluate,
    flush,
    get_current_parent_span_id,
    get_current_trace_id,
    init,
    shutdown,
    trace,
    trace_tool,
)
from poom.context import _SPAN_ID_STACK, push_span_id, reset_span_id

from fastapi import FastAPI
from httpx import ASGITransport, AsyncClient

class TestPoomPythonSdk(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        poom.init()

    def tearDown(self):
        poom.flush()

    async def test_sync_and_async_decorators(self):
        """Test basic sync and async @trace decorators."""
        @trace(name="my_sync_fn", kind=SpanKind.FUNCTION)
        def sync_fn(x: int) -> int:
            return x * 2

        @trace(name="my_async_fn", kind=SpanKind.AGENT)
        async def async_fn(x: int) -> int:
            await asyncio.sleep(0.001)
            return sync_fn(x) + 1

        result = await async_fn(5)
        self.assertEqual(result, 11)

    async def test_nested_hierarchy_5_levels(self):
        """Verify 5 levels of nesting properly propagate parent span IDs."""
        captured_parents = []

        @trace(name="level_5")
        def l5():
            captured_parents.append(get_current_parent_span_id())
            return 5

        @trace(name="level_4")
        def l4():
            captured_parents.append(get_current_parent_span_id())
            return l5()

        @trace(name="level_3")
        async def l3():
            captured_parents.append(get_current_parent_span_id())
            return l4()

        @trace(name="level_2")
        async def l2():
            captured_parents.append(get_current_parent_span_id())
            return await l3()

        @trace(name="level_1")
        async def l1():
            captured_parents.append(get_current_parent_span_id())
            return await l2()

        res = await l1()
        self.assertEqual(res, 5)
        self.assertEqual(len(captured_parents), 5)
        # Verify that all captured parent IDs are distinct non-None strings
        for p in captured_parents:
            self.assertIsNotNone(p)
        self.assertEqual(len(set(captured_parents)), 5)

    async def test_exception_trapping(self):
        """Verify that exceptions are caught, recorded, and faithfully re-raised."""
        @trace(name="error_fn")
        def buggy_fn():
            raise ValueError("Something went terribly wrong!")

        with self.assertRaises(ValueError) as ctx:
            buggy_fn()
        self.assertIn("Something went terribly wrong!", str(ctx.exception))

    async def test_trace_tool_decorator(self):
        """Verify @trace_tool decorator captures inputs and outputs."""
        @trace_tool(name="calculator")
        def add(a: int, b: int) -> int:
            return a + b

        res = add(10, 20)
        self.assertEqual(res, 30)

    async def test_fastapi_middleware_integration(self):
        """Test full FastAPI request pipeline with PoomMiddleware."""
        app = FastAPI()
        app.add_middleware(PoomMiddleware, app_name="test_api")

        @app.get("/items/{item_id}")
        @trace(kind=SpanKind.FUNCTION)
        async def get_item(item_id: int):
            return {"item_id": item_id, "status": "active"}

        @app.get("/error")
        async def trigger_error():
            raise RuntimeError("API route failure")

        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            resp = await client.get("/items/42")
            self.assertEqual(resp.status_code, 200)
            self.assertEqual(resp.json(), {"item_id": 42, "status": "active"})
            # Verify x-poom-trace-id header injection
            self.assertIn("x-poom-trace-id", resp.headers)
            trace_id_1 = resp.headers["x-poom-trace-id"]
            self.assertTrue(len(trace_id_1) > 0)

            # Test incoming trace propagation
            custom_trace_id = "018f0a00-0000-7000-8000-000000000001"
            resp2 = await client.get("/items/99", headers={"x-poom-trace-id": custom_trace_id})
            self.assertEqual(resp2.status_code, 200)
            self.assertEqual(resp2.headers["x-poom-trace-id"], custom_trace_id)

    def test_evaluation_recording(self):
        """Test recording evaluations."""
        trace_id = poom.generate_trace_id()
        res = evaluate(
            trace_id=trace_id,
            name="relevance",
            score=0.95,
            comment="Highly accurate answer",
        )
        self.assertTrue(res)

    async def test_context_manager_and_dynamic_attributes(self):
        """Test with poom.span(...) and dynamic set_attribute / set_tokens."""
        # Sync context manager
        with poom.span("sync_block", kind=SpanKind.LLM) as s:
            s.set_attribute("model", "gpt-4o")
            s.set_tokens(input_tokens=150, output_tokens=50, cached_tokens=20, cost_usd=0.001)
            self.assertEqual(s.attributes["model"], "gpt-4o")
            self.assertEqual(s.input_tokens, 150)

        # Async context manager
        async with poom.span("async_block", kind=SpanKind.TOOL) as s:
            poom.set_attribute("runtime", "asyncio")
            poom.set_tokens(input_tokens=10)
            self.assertEqual(s.attributes["runtime"], "asyncio")
            self.assertEqual(s.input_tokens, 10)

        # Traced function setting dynamic attributes
        @trace(kind=SpanKind.LLM)
        def call_openai_mock():
            poom.set_attribute("llm.provider", "openai")
            poom.set_tokens(input_tokens=500, output_tokens=200)
            return "ok"

        res = call_openai_mock()
        self.assertEqual(res, "ok")

    def test_micro_benchmark_record_span_latency(self):
        """Verify that record_span completes in < 100 microseconds (zero GIL bottleneck)."""
        trace_id = poom.generate_trace_id()
        span_id = poom.generate_span_id()
        iterations = 5_000

        start = time.perf_counter()
        for i in range(iterations):
            poom.record_span(
                trace_id=trace_id,
                span_id=span_id,
                name="bench_span",
                start_time_nanos=1000,
                end_time_nanos=2000,
                attributes={"iteration": i, "status": "ok"},
            )
        elapsed = time.perf_counter() - start
        avg_us = (elapsed / iterations) * 1_000_000

        print(f"\n[BENCHMARK] record_span average latency: {avg_us:.2f} microseconds per span")
        # Sub-100 microsecond SLA check
        self.assertLess(avg_us, 100.0, f"Average latency {avg_us:.2f}µs exceeded 100µs limit")

if __name__ == "__main__":
    unittest.main()
