use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use futures_util::{SinkExt, StreamExt};
use tempfile::tempdir;
use tokio::time::sleep;

use poom_protocol::{ClientMessage, ServerMessage};
use poom_transport::{IpcClient, IpcListener, ServerCodec, TransportError};
use poom_types::{SpanKind, SpanRecord, TraceId};

#[tokio::test]
async fn test_client_server_handshake_and_ping() {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("test_handshake.sock");

    let listener = IpcListener::bind(&sock_path).await.expect("Bind failed");

    // Spawn server accept task
    let server_handle = tokio::spawn(async move {
        let stream = listener.accept().await.expect("Accept failed");
        let mut framed = tokio_util::codec::Framed::new(stream, ServerCodec);

        // Receive Handshake
        let msg = framed.next().await.unwrap().unwrap();
        match msg {
            ClientMessage::Handshake { client_version, pid, .. } => {
                assert_eq!(client_version, "0.1.0");
                assert_eq!(pid, 4242);
                framed.send(ServerMessage::HandshakeAck {
                    server_version: "0.1.0".to_string(),
                    negotiated_protocol_version: 1,
                    max_batch_size: 1000,
                }).await.unwrap();
            }
            _ => panic!("Expected Handshake message"),
        }

        // Receive Ping
        let msg = framed.next().await.unwrap().unwrap();
        match msg {
            ClientMessage::Ping { timestamp_nanos } => {
                framed.send(ServerMessage::Pong { timestamp_nanos }).await.unwrap();
            }
            _ => panic!("Expected Ping message"),
        }
    });

    // Client connection
    let mut client = IpcClient::connect(&sock_path).await.expect("Connect failed");

    // Send Handshake
    client.send(ClientMessage::Handshake {
        client_version: "0.1.0".to_string(),
        pid: 4242,
        app_name: Some("test_app".to_string()),
    }).await.unwrap();

    let ack = client.next().await.unwrap().unwrap();
    assert!(matches!(ack, ServerMessage::HandshakeAck { .. }));

    // Send Ping
    let ping_time = 1_700_000_000_000_000;
    client.send(ClientMessage::Ping { timestamp_nanos: ping_time }).await.unwrap();

    let pong = client.next().await.unwrap().unwrap();
    assert_eq!(pong, ServerMessage::Pong { timestamp_nanos: ping_time });

    server_handle.await.unwrap();
}

#[tokio::test]
async fn test_concurrent_multi_client_streaming() {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("test_concurrent.sock");

    let listener = Arc::new(IpcListener::bind(&sock_path).await.expect("Bind failed"));
    let total_spans_received = Arc::new(AtomicUsize::new(0));

    let num_clients = 16;
    let batches_per_client = 10;
    let spans_per_batch = 10;
    let expected_spans = num_clients * batches_per_client * spans_per_batch;

    // Server loop accepting connections
    let listener_clone = Arc::clone(&listener);
    let counter_clone = Arc::clone(&total_spans_received);
    let server_handle = tokio::spawn(async move {
        for _ in 0..num_clients {
            let stream = listener_clone.accept().await.unwrap();
            let counter = Arc::clone(&counter_clone);

            tokio::spawn(async move {
                let mut framed = tokio_util::codec::Framed::new(stream, ServerCodec);
                while let Some(Ok(msg)) = framed.next().await {
                    match msg {
                        ClientMessage::IngestSpans(spans) => {
                            counter.fetch_add(spans.len(), Ordering::SeqCst);
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

    // Spawn concurrent clients
    let mut client_tasks = Vec::new();
    for client_id in 0..num_clients {
        let path = sock_path.clone();
        client_tasks.push(tokio::spawn(async move {
            let mut client = IpcClient::connect(&path).await.unwrap();

            for batch_id in 0..batches_per_client {
                let spans: Vec<SpanRecord> = (0..spans_per_batch)
                    .map(|i| {
                        let trace_id = TraceId::generate();
                        SpanRecord::root(trace_id, format!("client_{client_id}_batch_{batch_id}_span_{i}"), SpanKind::Function)
                    })
                    .collect();

                client.send(ClientMessage::IngestSpans(spans)).await.unwrap();
                let ack = client.next().await.unwrap().unwrap();
                assert!(matches!(ack, ServerMessage::IngestAck { .. }));
            }

            client.send(ClientMessage::Disconnect).await.unwrap();
        }));
    }

    for task in client_tasks {
        task.await.unwrap();
    }

    server_handle.await.unwrap();
    assert_eq!(total_spans_received.load(Ordering::SeqCst), expected_spans);
}

#[tokio::test]
async fn test_stale_socket_self_healing() {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("stale.sock");

    // Write a dummy non-socket file to simulate an abandoned crashed socket
    std::fs::write(&sock_path, b"abandoned dead socket content").unwrap();
    assert!(sock_path.exists());

    // Bind should probe, detect nobody is listening, unlink stale file, and bind successfully
    let listener = IpcListener::bind(&sock_path).await.expect("Should self-heal stale socket");

    // Client should now be able to connect
    let mut client = IpcClient::connect(&sock_path).await.expect("Client connect failed");
    let server_stream = listener.accept().await.expect("Accept failed");

    // Quick verification
    client.send(ClientMessage::Ping { timestamp_nanos: 12345 }).await.unwrap();
    let mut server_framed = tokio_util::codec::Framed::new(server_stream, ServerCodec);
    let msg = server_framed.next().await.unwrap().unwrap();
    assert_eq!(msg, ClientMessage::Ping { timestamp_nanos: 12345 });
}

#[tokio::test]
async fn test_address_in_use_detection() {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("active.sock");

    let _listener1 = IpcListener::bind(&sock_path).await.unwrap();

    // Second bind while listener1 is active must fail with AddressInUse
    let res = IpcListener::bind(&sock_path).await;
    assert!(matches!(res, Err(TransportError::AddressInUse(_))));
}

#[tokio::test]
async fn test_clean_on_drop() {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("droppable.sock");

    {
        let listener = IpcListener::bind(&sock_path).await.unwrap();
        assert!(sock_path.exists());
        drop(listener);
    }

    // Socket file must have been unlinked
    assert!(!sock_path.exists());
}

#[tokio::test]
async fn test_connect_with_retry() {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("delayed.sock");

    let path_clone = sock_path.clone();
    // Delayed listener creation
    tokio::spawn(async move {
        sleep(Duration::from_millis(60)).await;
        let _listener = IpcListener::bind(&path_clone).await.unwrap();
        // Hold open for 500ms
        sleep(Duration::from_millis(500)).await;
    });

    let client = IpcClient::connect_with_retry(
        &sock_path,
        5,
        Duration::from_millis(30),
        Duration::from_millis(200),
    ).await;

    assert!(client.is_ok(), "Client should succeed after server starts");
}
