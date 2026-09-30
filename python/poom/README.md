# Poom Python SDK

Ultra-low-overhead, local-first observability and trace instrumentation for Python and FastAPI.

## Installation

```bash
pip install poom
```

## Quickstart

```python
import poom
from poom import trace, trace_tool

# Initialize the tracer (connects to local Poom daemon)
poom.init()

@trace_tool()
def search_database(query: str):
    return {"results": [f"Document matching {query}"]}

@trace(kind=poom.SpanKind.AGENT)
async def run_agent(prompt: str):
    results = search_database(prompt)
    return f"Processed: {results}"
```

## FastAPI Auto-Instrumentation

```python
from fastapi import FastAPI
from poom import PoomMiddleware, trace

app = FastAPI()
app.add_middleware(PoomMiddleware, app_name="order_service")

@app.get("/items/{item_id}")
@trace()
async def get_item(item_id: int):
    return {"item_id": item_id, "name": "Widget"}
```
