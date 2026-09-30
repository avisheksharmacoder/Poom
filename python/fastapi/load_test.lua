-- ============================================================================
-- Poom Observability Studio — FastAPI Load Test Script for wrk
-- ============================================================================
-- Simulates realistic AI Agent traffic against FastAPI:
-- - 85% POST /api/chat (Multi-step Agent: Planning -> Vector Search -> Math -> LLM)
-- - 10% POST /api/batch-demo (Concurrent batch executions)
-- - 5%  GET  /api/error-demo (Error capture and stack traces)
--
-- Usage:
--   wrk -t4 -c20 -d30s --latency -s load_test.lua http://127.0.0.1:8000
-- ============================================================================

local prompts = {
    "Analyze recent enterprise revenue growth and forecast Q4 ARR",
    "Evaluate customer churn patterns across mid-market accounts",
    "Audit marketing campaign payback periods and blended CAC",
    "Calculate gross margin expansion after cloud migration",
    "Optimize inventory turnover across regional fulfillment hubs",
    "Synthesize competitive pricing intelligence from SaaS benchmarks",
    "Detect latency anomalies in payment processing microservices",
    "Benchmark vector database retrieval accuracy for hybrid search"
}

local models = {
    "gpt-4o",
    "claude-3-7-sonnet",
    "gemini-2.0-flash",
    "gpt-4o-mini"
}

local counter = 0
status_200 = 0
status_502 = 0
other_status = 0

-- Called by wrk on thread initialization
function init(args)
    counter = 0
    status_200 = 0
    status_502 = 0
    other_status = 0
end

-- Generates dynamic requests per HTTP connection
function request()
    counter = counter + 1
    
    local prompt_idx = (counter % #prompts) + 1
    local model_idx = (counter % #models) + 1
    
    local prompt = prompts[prompt_idx]
    local model = models[model_idx]
    local temperature = 0.3 + ((counter % 6) * 0.1)

    local roll = counter % 20

    -- Scenario 1: Error Demo (5% of requests)
    if roll == 0 then
        local headers = {}
        headers["User-Agent"] = "wrk-poom-loadtest/1.0"
        headers["Connection"] = "keep-alive"
        return wrk.format("GET", "/api/error-demo?simulate=true", headers)
    
    -- Scenario 2: Batch Execution (10% of requests)
    elseif roll == 1 or roll == 2 then
        local headers = {}
        headers["User-Agent"] = "wrk-poom-loadtest/1.0"
        headers["Content-Length"] = "0"
        headers["Connection"] = "keep-alive"
        return wrk.format("POST", "/api/batch-demo", headers)

    -- Scenario 3: Standard Multi-Step AI Agent Chat (85% of requests)
    else
        local body = string.format(
            '{"prompt": "%s", "model": "%s", "temperature": %.2f}',
            prompt, model, temperature
        )
        local headers = {}
        headers["Content-Type"] = "application/json"
        headers["Content-Length"] = tostring(string.len(body))
        headers["User-Agent"] = "wrk-poom-loadtest/1.0"
        headers["Connection"] = "keep-alive"
        return wrk.format("POST", "/api/chat", headers, body)
    end
end

local threads = {}

function setup(thread)
    thread:set("status_200", 0)
    thread:set("status_502", 0)
    thread:set("other_status", 0)
    table.insert(threads, thread)
end

-- Track HTTP responses per thread
function response(status, headers, body)
    if status == 200 then
        status_200 = status_200 + 1
    elseif status == 502 then
        status_502 = status_502 + 1
    else
        other_status = other_status + 1
    end
end

-- Summary statistics when benchmark finishes
function done(summary, latency, requests)
    local total_200 = 0
    local total_502 = 0
    local total_other = 0

    for _, thread in ipairs(threads) do
        total_200 = total_200 + (thread:get("status_200") or 0)
        total_502 = total_502 + (thread:get("status_502") or 0)
        total_other = total_other + (thread:get("other_status") or 0)
    end

    io.write("\n" .. string.rep("=", 65) .. "\n")
    io.write("📊 Poom Telemetry Ingestion Load Test Complete\n")
    io.write(string.rep("=", 65) .. "\n")
    io.write(string.format("  Total Requests:   %d\n", summary.requests))
    io.write(string.format("  Duration:         %.2fs\n", summary.duration / 1000000))
    io.write(string.format("  Requests/sec:     %.2f\n", summary.requests / (summary.duration / 1000000)))
    io.write(string.format("  Total Bytes:      %.2f MB\n", summary.bytes / (1024 * 1024)))
    io.write(string.format("  200 OK Traces:    %d\n", total_200))
    io.write(string.format("  502 Error Spans:  %d (Simulated error tracking)\n", total_502))
    if total_other > 0 then
        io.write(string.format("  Other Statuses:   %d\n", total_other))
    end
    io.write(string.rep("-", 65) .. "\n")
    io.write("  Latency Percentiles:\n")
    io.write(string.format("    50th (median):  %.2f ms\n", latency:percentile(50) / 1000))
    io.write(string.format("    90th:           %.2f ms\n", latency:percentile(90) / 1000))
    io.write(string.format("    99th:           %.2f ms\n", latency:percentile(99) / 1000))
    io.write("👉 Check Poom Observability Studio to watch all traces in real time!\n")
    io.write(string.rep("=", 65) .. "\n\n")
end
