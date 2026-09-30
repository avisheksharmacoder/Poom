use bytes::BytesMut;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use poom_protocol::{ClientMessage, FrameEncoder, FrameHeader, StreamFrameDecoder, HEADER_SIZE};
use poom_types::{SpanKind, SpanMetrics, SpanRecord, TraceId};

fn create_sample_span() -> SpanRecord {
    let trace_id = TraceId::generate();
    SpanRecord::root(trace_id, "chat_completion", SpanKind::Llm)
        .with_attribute("model", "gpt-4o")
        .with_attribute("temperature", 0.7)
        .with_attribute("user_id", "usr_12345")
        .with_metrics(
            SpanMetrics::new()
                .with_tokens(1024, 256)
                .with_cost(0.008)
                .with_cached_tokens(512),
        )
}

fn bench_header_encode_decode(c: &mut Criterion) {
    let header = FrameHeader::new(1024);
    let mut buf = [0u8; HEADER_SIZE];

    c.bench_function("header_encode", |b| {
        b.iter(|| {
            header.encode(black_box(&mut buf));
        })
    });

    c.bench_function("header_decode", |b| {
        b.iter(|| {
            let decoded = FrameHeader::decode(black_box(&buf)).unwrap();
            black_box(decoded);
        })
    });
}

fn bench_span_encode_decode(c: &mut Criterion) {
    let span = create_sample_span();
    let msg = ClientMessage::IngestSpans(vec![span]);
    let mut buf = BytesMut::with_capacity(4096);

    c.bench_function("single_span_encode", |b| {
        b.iter(|| {
            buf.clear();
            FrameEncoder::encode_client_message(black_box(&msg), &mut buf).unwrap();
        })
    });

    // Prepare encoded buffer for decoding bench
    buf.clear();
    FrameEncoder::encode_client_message(&msg, &mut buf).unwrap();

    c.bench_function("single_span_decode", |b| {
        b.iter(|| {
            let mut read_buf = buf.clone();
            let decoded = StreamFrameDecoder::decode_client_message(&mut read_buf)
                .unwrap()
                .unwrap();
            black_box(decoded);
        })
    });
}

fn bench_batch_spans_encode_decode(c: &mut Criterion) {
    let spans: Vec<SpanRecord> = (0..500).map(|_| create_sample_span()).collect();
    let msg = ClientMessage::IngestSpans(spans);
    let mut buf = BytesMut::with_capacity(1024 * 1024);

    c.bench_function("batch_500_spans_encode", |b| {
        b.iter(|| {
            buf.clear();
            FrameEncoder::encode_client_message(black_box(&msg), &mut buf).unwrap();
        })
    });

    buf.clear();
    FrameEncoder::encode_client_message(&msg, &mut buf).unwrap();

    c.bench_function("batch_500_spans_decode", |b| {
        b.iter(|| {
            let mut read_buf = buf.clone();
            let decoded = StreamFrameDecoder::decode_client_message(&mut read_buf)
                .unwrap()
                .unwrap();
            black_box(decoded);
        })
    });
}

criterion_group!(
    benches,
    bench_header_encode_decode,
    bench_span_encode_decode,
    bench_batch_spans_encode_decode
);
criterion_main!(benches);
