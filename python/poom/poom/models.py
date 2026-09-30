from enum import Enum
from typing import Optional, Dict, Any, List
from dataclasses import dataclass, field

class SpanKind(str, Enum):
    FUNCTION = "function"
    LLM = "llm"
    TOOL = "tool"
    CHAIN = "chain"
    AGENT = "agent"
    HTTP = "http"
    CUSTOM = "custom"

class SpanStatus(str, Enum):
    OK = "ok"
    ERROR = "error"
    UNSET = "unset"

@dataclass
class SpanMetrics:
    input_tokens: Optional[int] = None
    output_tokens: Optional[int] = None
    cached_tokens: Optional[int] = None
    reasoning_tokens: Optional[int] = None
    cost_usd: Optional[float] = None
