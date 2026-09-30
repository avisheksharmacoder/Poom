use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;

static CL100K_BPE: OnceLock<Option<CoreBPE>> = OnceLock::new();
static O200K_BPE: OnceLock<Option<CoreBPE>> = OnceLock::new();

/// High-performance lazy tokenizer engine.
///
/// BPE dictionaries are only loaded into RAM upon first demand, preserving
/// the sub-15 MB RSS profile during process boot.
pub struct LazyTokenizer;

impl LazyTokenizer {
    /// Counts tokens for the provided text and model identifier.
    pub fn count_tokens(model: &str, text: &str) -> u32 {
        if text.is_empty() {
            return 0;
        }

        let model_lower = model.to_lowercase();

        if model_lower.contains("gpt-4o") || model_lower.contains("o1") || model_lower.contains("o3") {
            if let Some(bpe) = O200K_BPE.get_or_init(|| tiktoken_rs::o200k_base().ok()) {
                return bpe.encode_with_special_tokens(text).len() as u32;
            }
        } else if model_lower.contains("gpt-4") || model_lower.contains("gpt-3.5") {
            if let Some(bpe) = CL100K_BPE.get_or_init(|| tiktoken_rs::cl100k_base().ok()) {
                return bpe.encode_with_special_tokens(text).len() as u32;
            }
        }

        // Fast character-ratio fallback for open-source / Anthropic / Gemini models
        // (average ~3.8 chars per subword token in standard English text)
        let chars = text.chars().count();
        (chars as f64 / 3.8).ceil().max(1.0) as u32
    }
}
