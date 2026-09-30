use bytes::BytesMut;
use proptest::prelude::*;

use poom_protocol::*;
use poom_types::*;

#[test]
fn test_client_messages_roundtrip() {
    let trace_id = TraceId::generate();
    let span = SpanRecord::root(trace_id, "agent_step", SpanKind::Agent)
        .with_attribute("env", "staging")
        .with_metrics(SpanMetrics::new().with_tokens(100, 20));

    let eval = EvaluationRecord::new(trace_id, Some(span.span_id), "accuracy", 0.95)
        .with_comment("Good answer");

    let messages = vec![
        ClientMessage::Handshake {
            client_version: "0.1.0".to_string(),
            pid: 12345,
            app_name: Some("fastapi_service".to_string()),
        },
        ClientMessage::IngestSpans(vec![span]),
        ClientMessage::IngestEvaluations(vec![eval]),
        ClientMessage::Ping {
            timestamp_nanos: 1_700_000_000_000_000_000,
        },
        ClientMessage::Disconnect,
    ];

    for original in messages {
        let mut buf = BytesMut::new();
        FrameEncoder::encode_client_message(&original, &mut buf).expect("Encoding must succeed");

        let decoded = StreamFrameDecoder::decode_client_message(&mut buf)
            .expect("Decoding must succeed")
            .expect("Message must be complete");

        assert_eq!(original, decoded);
        assert!(buf.is_empty(), "Buffer must be fully drained after decoding");
    }
}

#[test]
fn test_server_messages_roundtrip() {
    let responses = vec![
        ServerMessage::HandshakeAck {
            server_version: "0.1.0".to_string(),
            negotiated_protocol_version: 1,
            max_batch_size: 1000,
        },
        ServerMessage::IngestAck {
            spans_accepted: 42,
            evaluations_accepted: 2,
        },
        ServerMessage::Pong {
            timestamp_nanos: 1_700_000_000_000_000_000,
        },
        ServerMessage::Error {
            code: 4001,
            reason: "Batch queue full".to_string(),
        },
    ];

    for original in responses {
        let mut buf = BytesMut::new();
        FrameEncoder::encode_server_message(&original, &mut buf).expect("Encoding must succeed");

        let decoded = StreamFrameDecoder::decode_server_message(&mut buf)
            .expect("Decoding must succeed")
            .expect("Message must be complete");

        assert_eq!(original, decoded);
        assert!(buf.is_empty(), "Buffer must be fully drained after decoding");
    }
}

#[test]
fn test_stream_fragmentation_trickle() {
    // Encodes a message and feeds it to the decoder 1 byte at a time.
    let trace_id = TraceId::generate();
    let span = SpanRecord::root(trace_id, "llm_query", SpanKind::Llm)
        .with_attribute("model", "claude-3-5-sonnet");
    let msg = ClientMessage::IngestSpans(vec![span]);

    let mut full_wire_bytes = BytesMut::new();
    FrameEncoder::encode_client_message(&msg, &mut full_wire_bytes).unwrap();

    let total_len = full_wire_bytes.len();
    assert!(total_len > HEADER_SIZE);

    let mut streaming_buf = BytesMut::new();
    let mut decoded_msg = None;

    for (idx, byte) in full_wire_bytes.into_iter().enumerate() {
        streaming_buf.extend_from_slice(&[byte]);
        let res = StreamFrameDecoder::decode_client_message(&mut streaming_buf).unwrap();

        if idx + 1 < total_len {
            assert!(
                res.is_none(),
                "Decoder should return Ok(None) until all bytes arrive (idx: {}/{})",
                idx,
                total_len
            );
        } else {
            decoded_msg = res;
        }
    }

    assert_eq!(decoded_msg, Some(msg));
    assert!(streaming_buf.is_empty());
}

#[test]
fn test_multi_frame_burst() {
    // Concatenates multiple frames into a single buffer and extracts them sequentially
    let mut combined_buf = BytesMut::new();
    let count = 25;

    for i in 0..count {
        let msg = ClientMessage::Ping {
            timestamp_nanos: i as u64,
        };
        FrameEncoder::encode_client_message(&msg, &mut combined_buf).unwrap();
    }

    for i in 0..count {
        let decoded = StreamFrameDecoder::decode_client_message(&mut combined_buf)
            .unwrap()
            .expect("Frame must be available");

        assert_eq!(
            decoded,
            ClientMessage::Ping {
                timestamp_nanos: i as u64
            }
        );
    }

    assert!(combined_buf.is_empty());
    assert_eq!(
        StreamFrameDecoder::decode_client_message(&mut combined_buf).unwrap(),
        None
    );
}

#[test]
fn test_invalid_magic_rejection() {
    let mut buf = BytesMut::from(&[0xDE, 0xAD, 0x01, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04][..]);
    let err = StreamFrameDecoder::decode_client_message(&mut buf).unwrap_err();
    assert_eq!(err, ProtocolError::InvalidMagic(0xDE, 0xAD));
}

#[test]
fn test_unsupported_version_rejection() {
    // Magic "PM", version 99, flags 0, length 0
    let mut buf = BytesMut::from(&[0x50, 0x4D, 99, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]);
    let err = StreamFrameDecoder::decode_client_message(&mut buf).unwrap_err();
    assert_eq!(err, ProtocolError::UnsupportedVersion(99));
}

#[test]
fn test_oversized_frame_rejection() {
    // Length set to 100 MB (exceeds 64 MB guardrail)
    let len_bytes = (100 * 1024 * 1024u32).to_le_bytes();
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&MAGIC_BYTES);
    buf.extend_from_slice(&CURRENT_PROTOCOL_VERSION.to_le_bytes());
    buf.extend_from_slice(&[0, 0]); // flags
    buf.extend_from_slice(&len_bytes);

    let err = StreamFrameDecoder::decode_client_message(&mut buf).unwrap_err();
    assert!(matches!(err, ProtocolError::FrameTooLarge { .. }));
}

// ---------------------------------------------------------------------------
// Property-based roundtrip testing with proptest
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn proptest_ping_message_roundtrip(timestamp in any::<u64>()) {
        let msg = ClientMessage::Ping { timestamp_nanos: timestamp };
        let mut buf = BytesMut::new();
        FrameEncoder::encode_client_message(&msg, &mut buf).unwrap();
        let decoded = StreamFrameDecoder::decode_client_message(&mut buf).unwrap().unwrap();
        prop_assert_eq!(msg, decoded);
    }

    #[test]
    fn proptest_handshake_message_roundtrip(
        ver in "[a-zA-Z0-9._-]{1,20}",
        pid in any::<u32>(),
        app_name in proptest::option::of("[a-zA-Z0-9_]{1,30}")
    ) {
        let msg = ClientMessage::Handshake {
            client_version: ver,
            pid,
            app_name,
        };
        let mut buf = BytesMut::new();
        FrameEncoder::encode_client_message(&msg, &mut buf).unwrap();
        let decoded = StreamFrameDecoder::decode_client_message(&mut buf).unwrap().unwrap();
        prop_assert_eq!(msg, decoded);
    }
}
