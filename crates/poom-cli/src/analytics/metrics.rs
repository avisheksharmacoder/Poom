use std::collections::BTreeMap;
use poom_types::{SpanRecord, SpanStatus};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LatencyPercentiles {
    pub count: usize,
    pub min_ms: f64,
    pub max_ms: f64,
    pub avg_ms: f64,
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModelTokenVelocity {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd: f64,
    pub call_count: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnalyticsSummary {
    pub total_spans: usize,
    pub ok_spans: usize,
    pub error_spans: usize,
    pub overall_latency: LatencyPercentiles,
    pub model_tokens: BTreeMap<String, ModelTokenVelocity>,
    pub errors_by_type: BTreeMap<String, usize>,
    pub total_cost_usd: f64,
}

impl AnalyticsSummary {
    pub fn compute(spans: &[SpanRecord]) -> Self {
        if spans.is_empty() {
            return Self::default();
        }

        let mut durations_ms: Vec<f64> = Vec::with_capacity(spans.len());
        let mut ok_spans = 0;
        let mut error_spans = 0;
        let mut errors_by_type: BTreeMap<String, usize> = BTreeMap::new();
        let mut model_tokens: BTreeMap<String, ModelTokenVelocity> = BTreeMap::new();
        let mut total_cost_usd = 0.0;

        for span in spans {
            if let Some(nanos) = span.duration_nanos() {
                durations_ms.push(nanos as f64 / 1_000_000.0);
            }

            match &span.status {
                SpanStatus::Ok => ok_spans += 1,
                SpanStatus::Error { error_type, .. } => {
                    error_spans += 1;
                    *errors_by_type.entry(error_type.to_string()).or_insert(0) += 1;
                }
            }

            // Extract model name from attributes if LLM span
            let model_name = span
                .attributes
                .get("gen_ai.request.model")
                .or_else(|| span.attributes.get("model"))
                .or_else(|| span.attributes.get("llm.model"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            let entry = model_tokens.entry(model_name.to_string()).or_default();
            entry.call_count += 1;

            if let Some(tokens) = span.metrics.input_tokens {
                entry.input_tokens += tokens as u64;
                entry.total_tokens += tokens as u64;
            }
            if let Some(tokens) = span.metrics.output_tokens {
                entry.output_tokens += tokens as u64;
                entry.total_tokens += tokens as u64;
            }
            if let Some(tokens) = span.metrics.cached_tokens {
                entry.cached_tokens += tokens as u64;
            }
            if let Some(cost) = span.metrics.estimated_cost_usd {
                entry.estimated_cost_usd += cost;
                total_cost_usd += cost;
            }
        }

        let overall_latency = calculate_percentiles(&mut durations_ms);

        Self {
            total_spans: spans.len(),
            ok_spans,
            error_spans,
            overall_latency,
            model_tokens,
            errors_by_type,
            total_cost_usd,
        }
    }
}

pub fn calculate_percentiles(durations_ms: &mut [f64]) -> LatencyPercentiles {
    if durations_ms.is_empty() {
        return LatencyPercentiles::default();
    }

    durations_ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = durations_ms.len();
    let min_ms = durations_ms[0];
    let max_ms = durations_ms[n - 1];
    let sum: f64 = durations_ms.iter().sum();
    let avg_ms = sum / n as f64;

    let p50_ms = quantile(durations_ms, 0.50);
    let p90_ms = quantile(durations_ms, 0.90);
    let p95_ms = quantile(durations_ms, 0.95);
    let p99_ms = quantile(durations_ms, 0.99);

    LatencyPercentiles {
        count: n,
        min_ms,
        max_ms,
        avg_ms,
        p50_ms,
        p90_ms,
        p95_ms,
        p99_ms,
    }
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}
