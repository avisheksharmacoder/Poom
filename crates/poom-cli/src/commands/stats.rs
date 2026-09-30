use std::path::Path;
use std::time::Instant;

use redb::ReadableTable;
use poom_storage::schema::{SPANS_TABLE, TIME_INDEX_TABLE};
use poom_types::SpanRecord;

use crate::analytics::AnalyticsSummary;
use crate::error::CliResult;

pub fn execute_stats(db_path: &Path) -> CliResult<()> {
    if !db_path.exists() {
        println!("\x1b[33mDatabase does not exist yet at: {}\x1b[0m", db_path.display());
        println!("Run a traced application or start 'poom studio' / 'poom daemon' first.");
        return Ok(());
    }

    let start_time = Instant::now();
    let db = redb::Database::open(db_path)?;
    let file_size_bytes = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    let size_mb = file_size_bytes as f64 / (1024.0 * 1024.0);

    let (trace_count, span_count, spans) = {
        let read_txn = db.begin_read()?;
        let time_table = read_txn.open_table(TIME_INDEX_TABLE)?;
        let spans_table = read_txn.open_table(SPANS_TABLE)?;

        let t_count = time_table.iter()?.count();
        let s_count = spans_table.iter()?.count();

        let mut all_spans = Vec::with_capacity(s_count);
        for item in spans_table.iter()? {
            let (_, val) = item?;
            if let Ok(span) = postcard::from_bytes::<SpanRecord>(val.value()) {
                all_spans.push(span);
            }
        }

        (t_count, s_count, all_spans)
    };

    let summary = AnalyticsSummary::compute(&spans);
    let elapsed = start_time.elapsed();

    println!("\x1b[1;36m====================== Poom Storage & Telemetry Stats ======================\x1b[0m");
    println!("Database Path:      \x1b[37m{}\x1b[0m", db_path.display());
    println!("Database Size:      \x1b[32m{:.2} MB\x1b[0m ({} bytes)", size_mb, file_size_bytes);
    println!("Total Traces:       \x1b[1;37m{}\x1b[0m", trace_count);
    println!("Total Spans:        \x1b[1;37m{}\x1b[0m", span_count);
    println!(
        "Success / Errors:   \x1b[32m{} ok\x1b[0m / \x1b[31m{} error\x1b[0m (error rate: {:.1}%)",
        summary.ok_spans,
        summary.error_spans,
        if summary.total_spans > 0 {
            (summary.error_spans as f64 / summary.total_spans as f64) * 100.0
        } else {
            0.0
        }
    );
    println!("Total Estimated Cost: \x1b[1;32m${:.4} USD\x1b[0m", summary.total_cost_usd);

    if summary.overall_latency.count > 0 {
        println!("\n\x1b[1;34m--- Latency Percentiles (Duration across all spans) ---\x1b[0m");
        println!(
            "p50: {:.2}ms | p90: {:.2}ms | p95: {:.2}ms | p99: {:.2}ms | min: {:.2}ms | max: {:.2}ms | avg: {:.2}ms",
            summary.overall_latency.p50_ms,
            summary.overall_latency.p90_ms,
            summary.overall_latency.p95_ms,
            summary.overall_latency.p99_ms,
            summary.overall_latency.min_ms,
            summary.overall_latency.max_ms,
            summary.overall_latency.avg_ms
        );
    }

    if !summary.model_tokens.is_empty() {
        println!("\n\x1b[1;34m--- Token Breakdown by Model ---\x1b[0m");
        for (model, vel) in &summary.model_tokens {
            println!(
                "• \x1b[1;37m{:<24}\x1b[0m {:>5} calls | {:>7} in | {:>6} out | {:>6} cached | ${:.4}",
                model, vel.call_count, vel.input_tokens, vel.output_tokens, vel.cached_tokens, vel.estimated_cost_usd
            );
        }
    }

    if !summary.errors_by_type.is_empty() {
        println!("\n\x1b[1;31m--- Errors by Type ---\x1b[0m");
        for (err, count) in &summary.errors_by_type {
            println!("• {:<32} {}", err, count);
        }
    }

    println!("\x1b[90m----------------------------------------------------------------------------\x1b[0m");
    println!("\x1b[90mScanned in {:?}\x1b[0m", elapsed);

    Ok(())
}
