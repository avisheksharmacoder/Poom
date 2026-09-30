use std::collections::HashMap;

/// Financial pricing rates per 1,000,000 tokens in US Dollars.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelRates {
    pub prompt_per_1m: f64,
    pub completion_per_1m: f64,
    pub cached_per_1m: f64,
}

impl ModelRates {
    pub const fn new(prompt: f64, completion: f64, cached: f64) -> Self {
        Self {
            prompt_per_1m: prompt,
            completion_per_1m: completion,
            cached_per_1m: cached,
        }
    }
}

/// Dynamic pricing engine with built-in catalogs and model name normalization.
pub struct PricingEngine {
    rates: HashMap<String, ModelRates>,
}

impl Default for PricingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PricingEngine {
    /// Constructs a pricing engine initialized with standard model rates.
    pub fn new() -> Self {
        let mut rates = HashMap::new();

        // OpenAI Models
        rates.insert("gpt-4o".to_string(), ModelRates::new(2.50, 10.00, 1.25));
        rates.insert("gpt-4o-mini".to_string(), ModelRates::new(0.15, 0.60, 0.075));
        rates.insert("o1".to_string(), ModelRates::new(15.00, 60.00, 7.50));
        rates.insert("o1-mini".to_string(), ModelRates::new(3.00, 12.00, 1.50));
        rates.insert("o3-mini".to_string(), ModelRates::new(1.10, 4.40, 0.55));
        rates.insert("gpt-4-turbo".to_string(), ModelRates::new(10.00, 30.00, 10.00));
        rates.insert("gpt-3.5-turbo".to_string(), ModelRates::new(0.50, 1.50, 0.50));

        // Anthropic Claude
        rates.insert("claude-3-5-sonnet".to_string(), ModelRates::new(3.00, 15.00, 0.30));
        rates.insert("claude-3-5-haiku".to_string(), ModelRates::new(0.80, 4.00, 0.08));
        rates.insert("claude-3-haiku".to_string(), ModelRates::new(0.25, 1.25, 0.025));
        rates.insert("claude-3-opus".to_string(), ModelRates::new(15.00, 75.00, 1.50));

        // Google Gemini
        rates.insert("gemini-1.5-pro".to_string(), ModelRates::new(1.25, 5.00, 0.3125));
        rates.insert("gemini-1.5-flash".to_string(), ModelRates::new(0.075, 0.30, 0.01875));
        rates.insert("gemini-2.0-flash".to_string(), ModelRates::new(0.10, 0.40, 0.025));

        // DeepSeek
        rates.insert("deepseek-chat".to_string(), ModelRates::new(0.14, 0.28, 0.014));
        rates.insert("deepseek-r1".to_string(), ModelRates::new(0.55, 2.19, 0.14));

        // Meta Llama (hosted API baseline rates)
        rates.insert("llama-3.1-70b".to_string(), ModelRates::new(0.59, 0.79, 0.30));
        rates.insert("llama-3.1-8b".to_string(), ModelRates::new(0.05, 0.08, 0.02));

        Self { rates }
    }

    /// Normalizes raw model strings (e.g. "openai/gpt-4o-2024-08-06" -> "gpt-4o").
    pub fn normalize_model_name(&self, raw: &str) -> String {
        let mut s = raw.trim().to_lowercase();

        // Strip provider prefixes
        if let Some(pos) = s.find('/') {
            s = s[pos + 1..].to_string();
        }

        // Ordered prefix matching (specific before general)
        let canonical_keys = [
            "gpt-4o-mini",
            "gpt-4o",
            "o1-mini",
            "o1",
            "o3-mini",
            "gpt-4-turbo",
            "gpt-4",
            "gpt-3.5-turbo",
            "gpt-3.5",
            "claude-3-5-sonnet",
            "claude-3-5-haiku",
            "claude-3-haiku",
            "claude-3-opus",
            "gemini-2.0-flash",
            "gemini-1.5-flash",
            "gemini-1.5-pro",
            "deepseek-r1",
            "deepseek-chat",
            "llama-3.1-70b",
            "llama-3.1-8b",
        ];

        for key in canonical_keys {
            if s.starts_with(key) {
                return key.to_string();
            }
        }

        s
    }

    /// Computes the estimated financial cost in US Dollars.
    pub fn calculate_cost(
        &self,
        raw_model: &str,
        input_tokens: u32,
        output_tokens: u32,
        cached_tokens: Option<u32>,
    ) -> Option<f64> {
        let canonical = self.normalize_model_name(raw_model);
        let rates = self.rates.get(&canonical)?;

        let cached = cached_tokens.unwrap_or(0);
        let uncached_input = input_tokens.saturating_sub(cached);

        let cost = (cached as f64 * rates.cached_per_1m
            + uncached_input as f64 * rates.prompt_per_1m
            + output_tokens as f64 * rates.completion_per_1m)
            / 1_000_000.0;

        Some(cost)
    }

    /// Allows registering or overriding custom model rates.
    pub fn set_rates(&mut self, model: impl Into<String>, rates: ModelRates) {
        self.rates.insert(model.into().to_lowercase(), rates);
    }
}
