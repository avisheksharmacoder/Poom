use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use poom_storage::StorageReader;
use poom_types::TraceId;

use crate::commands::ExportFormat;
use crate::error::{CliError, CliResult};
use crate::export::{export_spans_to_json, export_spans_to_otlp, export_trace_to_json};

pub fn execute_export(
    db_path: &Path,
    trace_id_str: Option<String>,
    hours: Option<u64>,
    format: ExportFormat,
    output: Option<PathBuf>,
) -> CliResult<()> {
    if !db_path.exists() {
        return Err(CliError::InvalidArgument(format!(
            "Database does not exist at {}",
            db_path.display()
        )));
    }

    let db = redb::Database::open(db_path)?;
    let reader = StorageReader::new(&db);

    let output_str = if let Some(trace_str) = trace_id_str {
        let trace_id: TraceId = trace_str
            .parse()
            .map_err(|e| CliError::InvalidArgument(format!("Invalid Trace ID '{trace_str}': {e}")))?;

        let spans = reader.get_trace_spans(trace_id)?;
        if spans.is_empty() {
            return Err(CliError::TraceNotFound(trace_str));
        }

        match format {
            ExportFormat::Json => {
                let tree = reader
                    .get_trace_tree(trace_id)?
                    .ok_or_else(|| CliError::TraceNotFound(trace_str))?;
                let evals = reader.get_trace_evaluations(trace_id)?;
                export_trace_to_json(trace_id, &tree, evals)?
            }
            ExportFormat::Otlp => export_spans_to_otlp("poom_service", &spans)?,
        }
    } else {
        // Range export or recent traces export
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        let trace_list = if let Some(h) = hours {
            let window_nanos = h * 3600 * 1_000_000_000;
            let start_nanos = now_nanos.saturating_sub(window_nanos);
            reader.list_traces_in_range(start_nanos, now_nanos, 1000)?
        } else {
            reader.list_recent_traces(50, 0)?
        };

        let mut all_spans = Vec::new();
        for (_, t_id) in trace_list {
            let trace_spans = reader.get_trace_spans(t_id)?;
            all_spans.extend(trace_spans);
        }

        match format {
            ExportFormat::Json => export_spans_to_json(&all_spans)?,
            ExportFormat::Otlp => export_spans_to_otlp("poom_service", &all_spans)?,
        }
    };

    if let Some(out_path) = output {
        let mut file = File::create(&out_path)?;
        file.write_all(output_str.as_bytes())?;
        println!("\x1b[32mExport written to: {}\x1b[0m", out_path.display());
    } else {
        println!("{output_str}");
    }

    Ok(())
}
