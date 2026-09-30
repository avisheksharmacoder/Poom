use std::io;
use std::path::Path;

use poom_storage::StorageReader;
use poom_types::{SpanId, SpanRecord};

use crate::commands::DiffField;
use crate::diff::{render_terminal_diff, DiffResult};
use crate::error::{CliError, CliResult};

pub fn execute_diff(
    db_path: &Path,
    span1_str: &str,
    span2_str: &str,
    field: DiffField,
) -> CliResult<()> {
    if !db_path.exists() {
        return Err(CliError::InvalidArgument(format!(
            "Database does not exist at {}",
            db_path.display()
        )));
    }

    let span1_id: SpanId = span1_str
        .parse()
        .map_err(|e| CliError::InvalidArgument(format!("Invalid Span 1 ID '{span1_str}': {e}")))?;
    let span2_id: SpanId = span2_str
        .parse()
        .map_err(|e| CliError::InvalidArgument(format!("Invalid Span 2 ID '{span2_str}': {e}")))?;

    let db = redb::Database::open(db_path)?;
    let reader = StorageReader::new(&db);

    let span1 = reader
        .get_span(span1_id)?
        .ok_or_else(|| CliError::SpanNotFound(span1_str.to_string()))?;
    let span2 = reader
        .get_span(span2_id)?
        .ok_or_else(|| CliError::SpanNotFound(span2_str.to_string()))?;

    let mut stdout = io::stdout();

    let s1_prefix = if span1_str.len() >= 8 { &span1_str[..8] } else { span1_str };
    let s2_prefix = if span2_str.len() >= 8 { &span2_str[..8] } else { span2_str };

    match field {
        DiffField::Prompt => {
            let prompt1 = extract_prompt(&span1);
            let prompt2 = extract_prompt(&span2);
            let diff = DiffResult::compute(&prompt1, &prompt2);
            let title = format!("Prompt Diff: {s1_prefix} vs {s2_prefix}");
            render_terminal_diff(&mut stdout, &diff, Some(&title))?;
        }
        DiffField::Completion => {
            let comp1 = extract_completion(&span1);
            let comp2 = extract_completion(&span2);
            let diff = DiffResult::compute(&comp1, &comp2);
            let title = format!("Completion Diff: {s1_prefix} vs {s2_prefix}");
            render_terminal_diff(&mut stdout, &diff, Some(&title))?;
        }
        DiffField::All => {
            let p1 = extract_prompt(&span1);
            let p2 = extract_prompt(&span2);
            let p_diff = DiffResult::compute(&p1, &p2);
            render_terminal_diff(
                &mut stdout,
                &p_diff,
                Some(&format!("Prompt Diff ({s1_prefix} vs {s2_prefix})")),
            )?;

            let c1 = extract_completion(&span1);
            let c2 = extract_completion(&span2);
            let c_diff = DiffResult::compute(&c1, &c2);
            render_terminal_diff(
                &mut stdout,
                &c_diff,
                Some(&format!("Completion Diff ({s1_prefix} vs {s2_prefix})")),
            )?;
        }
    }

    Ok(())
}

fn extract_prompt(span: &SpanRecord) -> String {
    for key in &["prompt", "messages", "system_prompt", "query", "input"] {
        if let Some(val) = span.attributes.get(*key) {
            if let Some(s) = val.as_str() {
                return s.to_string();
            }
            return serde_json::to_string_pretty(&crate::export::json::attribute_to_json(val))
                .unwrap_or_default();
        }
    }
    String::new()
}

fn extract_completion(span: &SpanRecord) -> String {
    for key in &["completion", "response", "output", "result"] {
        if let Some(val) = span.attributes.get(*key) {
            if let Some(s) = val.as_str() {
                return s.to_string();
            }
            return serde_json::to_string_pretty(&crate::export::json::attribute_to_json(val))
                .unwrap_or_default();
        }
    }
    String::new()
}
