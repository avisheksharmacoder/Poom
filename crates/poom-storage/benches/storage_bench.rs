use criterion::{criterion_group, criterion_main, Criterion};
use tempfile::tempdir;

use poom_storage::{commit_batch, StorageEngine, StorageReader};
use poom_types::{SpanKind, SpanMetrics, SpanRecord, TraceId};

fn create_sample_span(trace_id: TraceId, idx: usize) -> SpanRecord {
    SpanRecord::root(trace_id, format!("benchmark_span_{idx}"), SpanKind::Llm)
        .with_attribute("model", "gpt-4o")
        .with_attribute("env", "bench")
        .with_metrics(SpanMetrics::new().with_tokens(800, 200).with_cost(0.006))
}

fn bench_storage_batch_commit(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("bench_batch.redb");
    let engine = StorageEngine::open(&db_path).unwrap();

    let trace_id = TraceId::generate();
    let batch: Vec<SpanRecord> = (0..500).map(|i| create_sample_span(trace_id, i)).collect();

    c.bench_function("storage_commit_500_spans", |b| {
        b.iter(|| {
            commit_batch(engine.database(), &batch, &[]).unwrap();
        })
    });
}

fn bench_storage_recent_traces_query(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("bench_pagination.redb");
    let engine = StorageEngine::open(&db_path).unwrap();

    // Pre-populate with 2,000 traces
    let mut spans = Vec::new();
    for i in 0..2000 {
        let trace_id = TraceId::generate();
        let mut span = create_sample_span(trace_id, i);
        span.start_time_unix_nanos = 1_000_000_000 + i as u64;
        spans.push(span);
    }
    commit_batch(engine.database(), &spans, &[]).unwrap();

    let reader = StorageReader::new(engine.database());

    c.bench_function("storage_list_recent_50_traces", |b| {
        b.iter(|| {
            let traces = reader.list_recent_traces(50, 0).unwrap();
            criterion::black_box(traces);
        })
    });
}

criterion_group!(
    benches,
    bench_storage_batch_commit,
    bench_storage_recent_traces_query
);
criterion_main!(benches);
