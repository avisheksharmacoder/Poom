pub mod engine;
pub mod terminal;

pub use engine::{ChangeType, DiffInlineChunk, DiffLine, DiffResult};
pub use terminal::render_terminal_diff;
