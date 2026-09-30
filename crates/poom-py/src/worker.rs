use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use bytes::BytesMut;
use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};

use poom_protocol::{ClientMessage, FrameEncoder};
use poom_transport::default_ipc_path;
use poom_types::{EvaluationRecord, SpanRecord};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use std::sync::Mutex;

pub enum WorkerCommand {
    RecordSpan(SpanRecord),
    RecordEvaluation(EvaluationRecord),
    Flush(SyncSender<()>),
    Shutdown,
}

/// Standalone background OS worker thread that drains spans without holding the Python GIL.
pub struct BackgroundWorker {
    sender: Sender<WorkerCommand>,
    thread_handle: Mutex<Option<JoinHandle<()>>>,
}

impl BackgroundWorker {
    /// Starts the background OS worker thread.
    pub fn start(socket_path: Option<PathBuf>, capacity: usize) -> Self {
        let (sender, receiver) = bounded::<WorkerCommand>(capacity);
        let path = socket_path.unwrap_or_else(default_ipc_path);

        let thread_handle = thread::spawn(move || {
            worker_loop(path, receiver);
        });

        Self {
            sender,
            thread_handle: Mutex::new(Some(thread_handle)),
        }
    }

    /// Non-blocking span dispatch into the lock-free crossbeam ring buffer.
    pub fn submit_span(&self, span: SpanRecord) -> bool {
        match self.sender.try_send(WorkerCommand::RecordSpan(span)) {
            Ok(_) => true,
            Err(TrySendError::Full(_)) => {
                // Buffer saturated, gracefully drop span to preserve application latency
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    /// Non-blocking evaluation dispatch.
    pub fn submit_evaluation(&self, eval: EvaluationRecord) -> bool {
        match self.sender.try_send(WorkerCommand::RecordEvaluation(eval)) {
            Ok(_) => true,
            Err(TrySendError::Full(_)) => false,
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    /// Flushes all queued records up to the specified timeout.
    pub fn flush(&self, timeout: Duration) -> bool {
        let (tx, rx) = std::sync::mpsc::sync_channel(0);
        if self.sender.send(WorkerCommand::Flush(tx)).is_ok() {
            rx.recv_timeout(timeout).is_ok()
        } else {
            false
        }
    }

    /// Shuts down the background worker and joins the worker thread.
    pub fn shutdown(&self) {
        let _ = self.sender.send(WorkerCommand::Shutdown);
        if let Ok(mut lock) = self.thread_handle.lock() {
            if let Some(handle) = lock.take() {
                let _ = handle.join();
            }
        }
    }
}

fn worker_loop(path: PathBuf, receiver: Receiver<WorkerCommand>) {
    let mut span_batch: Vec<SpanRecord> = Vec::with_capacity(1_000);
    let mut eval_batch: Vec<EvaluationRecord> = Vec::with_capacity(100);
    let mut last_flush = Instant::now();
    let flush_interval = Duration::from_millis(50);

    #[cfg(unix)]
    let mut stream: Option<UnixStream> = None;

    loop {
        // Attempt to drain up to 1,000 items with a short timeout
        match receiver.recv_timeout(Duration::from_millis(10)) {
            Ok(WorkerCommand::RecordSpan(span)) => {
                span_batch.push(span);
                if span_batch.len() >= 1_000 {
                    #[cfg(unix)]
                    flush_to_socket(&path, &mut stream, &mut span_batch, &mut eval_batch);
                    last_flush = Instant::now();
                }
            }
            Ok(WorkerCommand::RecordEvaluation(eval)) => {
                eval_batch.push(eval);
                if span_batch.len() + eval_batch.len() >= 1_000 {
                    #[cfg(unix)]
                    flush_to_socket(&path, &mut stream, &mut span_batch, &mut eval_batch);
                    last_flush = Instant::now();
                }
            }
            Ok(WorkerCommand::Flush(ack_tx)) => {
                #[cfg(unix)]
                flush_to_socket(&path, &mut stream, &mut span_batch, &mut eval_batch);
                last_flush = Instant::now();
                let _ = ack_tx.send(());
            }
            Ok(WorkerCommand::Shutdown) => {
                #[cfg(unix)]
                {
                    flush_to_socket(&path, &mut stream, &mut span_batch, &mut eval_batch);
                    if let Some(ref mut s) = stream {
                        let mut disconnect_buf = BytesMut::new();
                        if FrameEncoder::encode_client_message(&ClientMessage::Disconnect, &mut disconnect_buf).is_ok() {
                            let _ = s.write_all(&disconnect_buf);
                            let _ = s.flush();
                        }
                    }
                }
                break;
            }
            Err(_) => {
                // Timeout elapsed, check periodic flush
                if (!span_batch.is_empty() || !eval_batch.is_empty()) && last_flush.elapsed() >= flush_interval {
                    #[cfg(unix)]
                    flush_to_socket(&path, &mut stream, &mut span_batch, &mut eval_batch);
                    last_flush = Instant::now();
                }
            }
        }
    }
}

#[cfg(unix)]
fn flush_to_socket(
    path: &PathBuf,
    stream: &mut Option<UnixStream>,
    spans: &mut Vec<SpanRecord>,
    evals: &mut Vec<EvaluationRecord>,
) {
    if spans.is_empty() && evals.is_empty() {
        return;
    }

    // Ensure connection is established
    if stream.is_none() {
        match UnixStream::connect(path) {
            Ok(new_stream) => {
                let _ = new_stream.set_nonblocking(false);
                let _ = new_stream.set_write_timeout(Some(Duration::from_millis(500)));

                // Send initial Handshake
                let mut handshake_buf = BytesMut::new();
                let handshake_msg = ClientMessage::Handshake {
                    client_version: "0.1.0".to_string(),
                    pid: std::process::id(),
                    app_name: Some("python_client".to_string()),
                };
                if FrameEncoder::encode_client_message(&handshake_msg, &mut handshake_buf).is_ok() {
                    let mut s = new_stream;
                    if s.write_all(&handshake_buf).is_ok() {
                        *stream = Some(s);
                    }
                }
            }
            Err(_) => {}
        }
    }

    if let Some(ref mut s) = stream {
        let mut buf = BytesMut::new();

        if !spans.is_empty() {
            let msg = ClientMessage::IngestSpans(std::mem::take(spans));
            if FrameEncoder::encode_client_message(&msg, &mut buf).is_ok() {
                if s.write_all(&buf).is_err() {
                    // Broken pipe, reset connection
                    *stream = None;
                    return;
                }
            }
        }

        buf.clear();
        if !evals.is_empty() {
            let msg = ClientMessage::IngestEvaluations(std::mem::take(evals));
            if FrameEncoder::encode_client_message(&msg, &mut buf).is_ok() {
                if s.write_all(&buf).is_err() {
                    *stream = None;
                    return;
                }
            }
        }
    } else {
        // Daemon not reachable; clear buffers so memory is bounded
        spans.clear();
        evals.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc;
    use poom_protocol::StreamFrameDecoder;
    use poom_types::{SpanId, SpanKind, SpanStatus, TraceId};

    #[test]
    fn test_background_worker_flow() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sock_path = temp_dir.path().join("test_poom.sock");

        let listener = UnixListener::bind(&sock_path).unwrap();
        let (tx, rx) = mpsc::channel();

        // Spawn mock server thread
        let server_handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut read_buf = BytesMut::new();
            let mut scratch = [0u8; 1024];
            let mut received_messages = Vec::new();

            loop {
                match stream.read(&mut scratch) {
                    Ok(0) => break,
                    Ok(n) => {
                        read_buf.extend_from_slice(&scratch[..n]);
                        while let Ok(Some(msg)) = StreamFrameDecoder::decode_client_message(&mut read_buf) {
                            received_messages.push(msg);
                            if received_messages.len() >= 2 {
                                let _ = tx.send(received_messages);
                                return;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Start worker
        let worker = BackgroundWorker::start(Some(sock_path), 500);

        let span = SpanRecord {
            trace_id: TraceId::generate(),
            span_id: SpanId::generate(),
            parent_span_id: None,
            name: "test_span".into(),
            kind: SpanKind::Function,
            start_time_unix_nanos: 1_000_000,
            end_time_unix_nanos: Some(2_000_000),
            status: SpanStatus::Ok,
            attributes: std::collections::BTreeMap::new(),
            events: Vec::new(),
            metrics: poom_types::SpanMetrics::default(),
        };

        assert!(worker.submit_span(span));
        assert!(worker.flush(Duration::from_secs(2)));
        worker.shutdown();

        let received = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        server_handle.join().unwrap();

        assert_eq!(received.len(), 2);
        match &received[0] {
            ClientMessage::Handshake { client_version, .. } => {
                assert_eq!(client_version, "0.1.0");
            }
            other => panic!("Expected Handshake, got {:?}", other),
        }
        match &received[1] {
            ClientMessage::IngestSpans(spans) => {
                assert_eq!(spans.len(), 1);
                assert_eq!(spans[0].name.as_str(), "test_span");
            }
            other => panic!("Expected IngestSpans, got {:?}", other),
        }
    }
}

