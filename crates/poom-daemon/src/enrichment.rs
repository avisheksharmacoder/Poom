use poom_types::{SpanKind, SpanRecord};
use crate::cost::PricingEngine;
use crate::tokenizer::LazyTokenizer;

/// Ingestion stage responsible for token counting and financial cost calculations.
pub struct EnrichmentEngine {
    pricing: PricingEngine,
}

impl Default for EnrichmentEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EnrichmentEngine {
    /// Constructs an enrichment engine with standard pricing catalogs.
    pub fn new() -> Self {
        Self {
            pricing: PricingEngine::new(),
        }
    }

    /// Enriches an in-flight span record with token metrics and USD costs.
    pub fn enrich_span(&self, span: &mut SpanRecord, auto_tokenize: bool) {
        if span.kind != SpanKind::Llm {
            return;
        }

        let model_str = span
            .attributes
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("gpt-4o")
            .to_string();

        // 1. Auto-tokenization if metrics are missing
        if auto_tokenize {
            if span.metrics.input_tokens.is_none() {
                if let Some(prompt_text) = find_attribute_text(span, &["prompt", "input", "user_message"]) {
                    let count = LazyTokenizer::count_tokens(&model_str, &prompt_text);
                    span.metrics.input_tokens = Some(count);
                }
            }

            if span.metrics.output_tokens.is_none() {
                if let Some(completion_text) = find_attribute_text(span, &["completion", "output", "response", "assistant_message"]) {
                    let count = LazyTokenizer::count_tokens(&model_str, &completion_text);
                    span.metrics.output_tokens = Some(count);
                }
            }
        }

        // 2. Normalize total tokens
        span.metrics.normalize_totals();

        // 3. Compute financial cost
        if span.metrics.estimated_cost_usd.is_none() {
            if let (Some(input_tokens), Some(output_tokens)) =
                (span.metrics.input_tokens, span.metrics.output_tokens)
            {
                if let Some(cost) = self.pricing.calculate_cost(
                    &model_str,
                    input_tokens,
                    output_tokens,
                    span.metrics.cached_tokens,
                ) {
                    span.metrics.estimated_cost_usd = Some(cost);
                }
            }
        }
    }

    /// Returns a reference to the underlying pricing engine.
    pub fn pricing(&self) -> &PricingEngine {
        &self.pricing
    }
}

fn find_attribute_text(span: &SpanRecord, keys: &[&str]) -> Option<String> {
    for &key in keys {
        if let Some(val) = span.attributes.get(key) {
            if let Some(s) = val.as_str() {
                return Some(s.to_string());
            }
        }
    }
    None
}
