use criterion::{criterion_group, criterion_main, Criterion};
use futures_util::{SinkExt, StreamExt};
use tempfile::tempdir;
use tokio::runtime::Runtime;

use poom_protocol::{ClientMessage, ServerMessage};
use poom_transport::{IpcClient, IpcListener, ServerCodec};
use poom_types::{SpanKind, SpanRecord, TraceId};

fn bench_ipc_ping_pong_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("bench_ping.sock");

    let listener = rt.block_on(async {
        IpcListener::bind(&sock_path).await.unwrap()
    });

    // Server loop
    rt.spawn(async move {
        while let Ok(stream) = listener.accept().await {
            tokio::spawn(async move {
                let mut framed = tokio_util::codec::Framed::new(stream, ServerCodec);
                while let Some(Ok(msg)) = framed.next().await {
                    match msg {
                        ClientMessage::Ping { timestamp_nanos } => {
                            let _ = framed.send(ServerMessage::Pong { timestamp_nanos }).await;
                        }
                        ClientMessage::Disconnect => break,
                        _ => {}
                    }
                }
            });
        }
    });

    let mut client = rt.block_on(async {
        IpcClient::connect(&sock_path).await.unwrap()
    });

    c.bench_function("ipc_uds_ping_pong_latency", |b| {
        b.iter(|| {
            rt.block_on(async {
                client.send(ClientMessage::Ping { timestamp_nanos: 1234 }).await.unwrap();
                let resp = client.next().await.unwrap().unwrap();
                criterion::black_box(resp);
            });
        })
    });
}

fn bench_ipc_span_batch_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("bench_batch.sock");

    let listener = rt.block_on(async {
        IpcListener::bind(&sock_path).await.unwrap()
    });

    rt.spawn(async move {
        while let Ok(stream) = listener.accept().await {
            tokio::spawn(async move {
                let mut framed = tokio_util::codec::Framed::new(stream, ServerCodec);
                while let Some(Ok(msg)) = framed.next().await {
                    match msg {
                        ClientMessage::IngestSpans(spans) => {
                            let _ = framed.send(ServerMessage::IngestAck {
                                spans_accepted: spans.len() as u32,
                                evaluations_accepted: 0,
                            }).await;
                        }
                        ClientMessage::Disconnect => break,
                        _ => {}
                    }
                }
            });
        }
    });

    let mut client = rt.block_on(async {
        IpcClient::connect(&sock_path).await.unwrap()
    });

    let spans: Vec<SpanRecord> = (0..100)
        .map(|i| {
            let trace_id = TraceId::generate();
            SpanRecord::root(trace_id, format!("batch_span_{i}"), SpanKind::Function)
        })
        .collect();
    let batch_msg = ClientMessage::IngestSpans(spans);

    c.bench_function("ipc_uds_batch_100_spans", |b| {
        b.iter(|| {
            rt.block_on(async {
                client.send(batch_msg.clone()).await.unwrap();
                let resp = client.next().await.unwrap().unwrap();
                criterion::black_box(resp);
            });
        })
    });
}

criterion_group!(
    benches,
    bench_ipc_ping_pong_latency,
    bench_ipc_span_batch_throughput
);
criterion_main!(benches);
