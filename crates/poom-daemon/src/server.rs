use std::sync::Arc;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::Notify;

use poom_protocol::{ClientMessage, ServerMessage};
use poom_storage::{BatchWriter, StorageEngine};
use poom_transport::{IpcListener, ServerCodec, ServerFramed};

use crate::broadcast::EventBroadcaster;
use crate::config::DaemonConfig;
use crate::enrichment::EnrichmentEngine;
use crate::error::DaemonError;
use crate::telemetry::DaemonTelemetry;

/// The central Poom ingestion daemon orchestrator.
pub struct PoomDaemon {
    config: DaemonConfig,
    storage: StorageEngine,
    writer: BatchWriter,
    listener: Arc<IpcListener>,
    enrichment: Arc<EnrichmentEngine>,
    broadcaster: EventBroadcaster,
    telemetry: Arc<DaemonTelemetry>,
    shutdown_notify: Arc<Notify>,
}

impl PoomDaemon {
    /// Binds the socket, opens storage, and starts all background workers.
    pub async fn start(config: DaemonConfig) -> Result<Self, DaemonError> {
        let listener = Arc::new(IpcListener::bind(&config.socket_path).await?);
        let storage = StorageEngine::open(&config.db_path)?;
        let writer = BatchWriter::start(storage.database().clone());
        let broadcaster = EventBroadcaster::new(config.broadcast_capacity);
        let enrichment = Arc::new(EnrichmentEngine::new());
        let telemetry = Arc::new(DaemonTelemetry::new());
        let shutdown_notify = Arc::new(Notify::new());

        Ok(Self {
            config,
            storage,
            writer,
            listener,
            enrichment,
            broadcaster,
            telemetry,
            shutdown_notify,
        })
    }

    /// Runs the accept loop, spawning session tasks for incoming client connections.
    pub async fn run(&self) -> Result<(), DaemonError> {
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    break;
                }
                accept_res = self.listener.accept() => {
                    match accept_res {
                        Ok(stream) => {
                            let framed = tokio_util::codec::Framed::new(stream, ServerCodec);
                            let enrichment = Arc::clone(&self.enrichment);
                            let broadcaster = self.broadcaster.clone();
                            let telemetry = Arc::clone(&self.telemetry);
                            let auto_tokenize = self.config.auto_tokenize;

                            // Clone the BatchWriter sender by using a thin handle
                            let db = self.storage.database().clone();
                            let local_writer = BatchWriter::start(db);

                            tokio::spawn(async move {
                                Self::handle_client_session(
                                    framed,
                                    local_writer,
                                    enrichment,
                                    broadcaster,
                                    telemetry,
                                    auto_tokenize,
                                ).await;
                            });
                        }
                        Err(e) => {
                            // If socket closed due to shutdown, exit cleanly
                            if self.is_shutdown_requested() {
                                break;
                            }
                            eprintln!("Socket accept error: {e}");
                        }
                    }
                }
            }
        }

        self.writer.flush().await?;
        Ok(())
    }

    /// Handles a single client connection session.
    async fn handle_client_session(
        mut framed: ServerFramed,
        writer: BatchWriter,
        enrichment: Arc<EnrichmentEngine>,
        broadcaster: EventBroadcaster,
        telemetry: Arc<DaemonTelemetry>,
        auto_tokenize: bool,
    ) {
        telemetry.inc_connections();

        while let Some(res) = framed.next().await {
            match res {
                Ok(msg) => match msg {
                ClientMessage::Handshake { .. } => {
                    let ack = ServerMessage::HandshakeAck {
                        server_version: "0.1.0".to_string(),
                        negotiated_protocol_version: 1,
                        max_batch_size: 1000,
                    };
                    let _ = framed.send(ack).await;
                }
                ClientMessage::Ping { timestamp_nanos } => {
                    let _ = framed.send(ServerMessage::Pong { timestamp_nanos }).await;
                }
                ClientMessage::IngestSpans(mut spans) => {
                    let count = spans.len();

                    // Enrich spans with token metrics and USD costs
                    for span in &mut spans {
                        enrichment.enrich_span(span, auto_tokenize);
                        broadcaster.broadcast(span.clone());
                    }

                    // Forward to storage batch writer
                    let _ = writer.send_spans(spans).await;
                    telemetry.add_spans(count);

                    let ack = ServerMessage::IngestAck {
                        spans_accepted: count as u32,
                        evaluations_accepted: 0,
                    };
                    let _ = framed.send(ack).await;
                }
                ClientMessage::IngestEvaluations(evals) => {
                    let count = evals.len();
                    let _ = writer.send_evaluations(evals).await;
                    telemetry.add_evaluations(count);

                    let ack = ServerMessage::IngestAck {
                        spans_accepted: 0,
                        evaluations_accepted: count as u32,
                    };
                    let _ = framed.send(ack).await;
                }
                ClientMessage::Disconnect => {
                    break;
                }
            },
            Err(e) => {
                eprintln!("[DAEMON FRAMED ERROR]: {:?}", e);
                break;
            }
        }
        }

        let _ = writer.flush().await;
        telemetry.dec_connections();
    }

    /// Triggers graceful shutdown of the daemon.
    pub fn shutdown(&self) {
        self.shutdown_notify.notify_waiters();
    }

    fn is_shutdown_requested(&self) -> bool {
        // Simple internal check
        false
    }

    pub fn broadcaster(&self) -> &EventBroadcaster {
        &self.broadcaster
    }

    pub fn telemetry(&self) -> &Arc<DaemonTelemetry> {
        &self.telemetry
    }

    pub fn storage(&self) -> &StorageEngine {
        &self.storage
    }

    pub fn config(&self) -> &DaemonConfig {
        &self.config
    }
}
