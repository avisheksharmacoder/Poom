use std::io::{self, Write};
use crate::diff::engine::{ChangeType, DiffResult};

/// Renders a formatted ANSI colored diff to a standard writer.
pub fn render_terminal_diff<W: Write>(writer: &mut W, diff: &DiffResult, title: Option<&str>) -> io::Result<()> {
    if let Some(t) = title {
        writeln!(writer, "\x1b[1;36m=== {t} ===\x1b[0m")?;
    }

    writeln!(
        writer,
        "\x1b[90mSimilarity: {:.1}% | \x1b[32m+{} additions\x1b[90m | \x1b[31m-{} deletions\x1b[90m | {} unchanged lines\x1b[0m",
        diff.similarity_ratio * 100.0,
        diff.additions,
        diff.deletions,
        diff.unchanged
    )?;
    writeln!(writer, "\x1b[90m{}\x1b[0m", "-".repeat(70))?;

    for line in &diff.lines {
        match line.tag {
            ChangeType::Equal => {
                let idx_str = line
                    .old_index
                    .map(|i| format!("{:3}", i + 1))
                    .unwrap_or_else(|| "   ".to_string());
                writeln!(writer, "\x1b[90m{}  \x1b[0m {}", idx_str, line.content)?;
            }
            ChangeType::Delete => {
                let idx_str = line
                    .old_index
                    .map(|i| format!("{:3}", i + 1))
                    .unwrap_or_else(|| "   ".to_string());

                if line.inline_chunks.is_empty() {
                    writeln!(writer, "\x1b[31m{} - {}\x1b[0m", idx_str, line.content)?;
                } else {
                    write!(writer, "\x1b[31m{} - \x1b[0m", idx_str)?;
                    for chunk in &line.inline_chunks {
                        if chunk.tag == ChangeType::Delete {
                            write!(writer, "\x1b[41;37m{}\x1b[0m", chunk.text)?;
                        } else {
                            write!(writer, "\x1b[31m{}\x1b[0m", chunk.text)?;
                        }
                    }
                    writeln!(writer)?;
                }
            }
            ChangeType::Insert => {
                let idx_str = line
                    .new_index
                    .map(|i| format!("{:3}", i + 1))
                    .unwrap_or_else(|| "   ".to_string());

                if line.inline_chunks.is_empty() {
                    writeln!(writer, "\x1b[32m{} + {}\x1b[0m", idx_str, line.content)?;
                } else {
                    write!(writer, "\x1b[32m{} + \x1b[0m", idx_str)?;
                    for chunk in &line.inline_chunks {
                        if chunk.tag == ChangeType::Insert {
                            write!(writer, "\x1b[42;30m{}\x1b[0m", chunk.text)?;
                        } else {
                            write!(writer, "\x1b[32m{}\x1b[0m", chunk.text)?;
                        }
                    }
                    writeln!(writer)?;
                }
            }
        }
    }

    writeln!(writer, "\x1b[90m{}\x1b[0m", "-".repeat(70))?;
    Ok(())
}
