use std::sync::Arc;
use std::time::Duration;
use redb::{Database, WriteTransaction};
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;

use poom_types::{EvaluationRecord, SpanRecord};
use crate::error::StorageError;
use crate::keys::{encode_tag_index_key, encode_time_index_key};
use crate::schema::*;

/// Inserts a span record into an open write transaction across all indexed tables.
pub fn insert_span_in_txn(
    txn: &WriteTransaction,
    span: &SpanRecord,
) -> Result<(), StorageError> {
    let payload_bytes = postcard::to_stdvec(span)?;

    // 1. Primary span storage
    {
        let mut spans_table = txn.open_table(SPANS_TABLE)?;
        spans_table.insert(span.span_id.as_bytes(), payload_bytes.as_slice())?;
    }

    // 2. Trace -> Span multimap
    {
        let mut trace_spans = txn.open_multimap_table(TRACE_SPANS_TABLE)?;
        trace_spans.insert(span.trace_id.as_bytes(), span.span_id.as_bytes())?;
    }

    // 3. Chronological time index (for root spans only)
    if span.is_root() {
        let mut time_index = txn.open_table(TIME_INDEX_TABLE)?;
        let key = encode_time_index_key(span.start_time_unix_nanos, span.trace_id);
        time_index.insert(&key, ())?;
    }

    // 4. Parent -> Child span multimap
    if let Some(parent_id) = span.parent_span_id {
        let mut span_children = txn.open_multimap_table(SPAN_CHILDREN_TABLE)?;
        span_children.insert(parent_id.as_bytes(), span.span_id.as_bytes())?;
    }

    // 5. Attribute tag index
    if !span.attributes.is_empty() {
        let mut tag_table = txn.open_table(TAG_INDEX_TABLE)?;
        for (k, v) in &span.attributes {
            if let Some(val_str) = v.as_str() {
                let tag_key = encode_tag_index_key(k.as_str(), val_str, span.trace_id);
                tag_table.insert(tag_key.as_slice(), ())?;
            }
        }
    }

    Ok(())
}

/// Inserts an evaluation record into an open write transaction across evaluation tables.
pub fn insert_evaluation_in_txn(
    txn: &WriteTransaction,
    eval: &EvaluationRecord,
) -> Result<(), StorageError> {
    let payload_bytes = postcard::to_stdvec(eval)?;

    // 1. Primary evaluation storage
    {
        let mut evals_table = txn.open_table(EVALUATIONS_TABLE)?;
        evals_table.insert(eval.evaluation_id.as_bytes(), payload_bytes.as_slice())?;
    }

    // 2. Trace -> Evaluation multimap
    {
        let mut trace_evals = txn.open_multimap_table(TRACE_EVALUATIONS_TABLE)?;
        trace_evals.insert(eval.trace_id.as_bytes(), eval.evaluation_id.as_bytes())?;
    }

    Ok(())
}

/// Atomically inserts and commits a batch of spans and evaluations.
pub fn commit_batch(
    db: &Database,
    spans: &[SpanRecord],
    evaluations: &[EvaluationRecord],
) -> Result<(), StorageError> {
    if spans.is_empty() && evaluations.is_empty() {
        return Ok(());
    }

    let write_txn = db.begin_write()?;
    for span in spans {
        insert_span_in_txn(&write_txn, span)?;
    }
    for eval in evaluations {
        insert_evaluation_in_txn(&write_txn, eval)?;
    }
    write_txn.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Micro-Batch Writer Actor with Dual Triggers (1,000 items / 50ms)
// ---------------------------------------------------------------------------

enum WriteCommand {
    IngestSpans(Vec<SpanRecord>),
    IngestEvaluations(Vec<EvaluationRecord>),
    Flush(oneshot::Sender<()>),
}

/// Asynchronous micro-batch writer coalescing high-frequency writes into ACID transactions.
pub struct BatchWriter {
    sender: mpsc::Sender<WriteCommand>,
}

impl BatchWriter {
    /// Starts the background micro-batching worker task.
    pub fn start(db: Arc<Database>) -> Self {
        let (sender, mut receiver) = mpsc::channel::<WriteCommand>(10_000);

        tokio::spawn(async move {
            let mut span_batch = Vec::with_capacity(1_000);
            let mut eval_batch = Vec::with_capacity(100);
            let flush_interval = Duration::from_millis(50);

            loop {
                tokio::select! {
                    cmd_opt = receiver.recv() => {
                        match cmd_opt {
                            Some(WriteCommand::IngestSpans(mut spans)) => {
                                span_batch.append(&mut spans);
                                if span_batch.len() >= 1_000 {
                                    let _ = commit_batch(&db, &span_batch, &eval_batch);
                                    span_batch.clear();
                                    eval_batch.clear();
                                }
                            }
                            Some(WriteCommand::IngestEvaluations(mut evals)) => {
                                eval_batch.append(&mut evals);
                                if span_batch.len() + eval_batch.len() >= 1_000 {
                                    let _ = commit_batch(&db, &span_batch, &eval_batch);
                                    span_batch.clear();
                                    eval_batch.clear();
                                }
                            }
                            Some(WriteCommand::Flush(tx)) => {
                                if !span_batch.is_empty() || !eval_batch.is_empty() {
                                    let _ = commit_batch(&db, &span_batch, &eval_batch);
                                    span_batch.clear();
                                    eval_batch.clear();
                                }
                                let _ = tx.send(());
                            }
                            None => {
                                // Channel closed, final drain
                                if !span_batch.is_empty() || !eval_batch.is_empty() {
                                    let _ = commit_batch(&db, &span_batch, &eval_batch);
                                }
                                break;
                            }
                        }
                    }
                    _ = sleep(flush_interval) => {
                        if !span_batch.is_empty() || !eval_batch.is_empty() {
                            let _ = commit_batch(&db, &span_batch, &eval_batch);
                            span_batch.clear();
                            eval_batch.clear();
                        }
                    }
                }
            }
        });

        Self { sender }
    }

    /// Queues spans for ingestion.
    pub async fn send_spans(&self, spans: Vec<SpanRecord>) -> Result<(), StorageError> {
        self.sender
            .send(WriteCommand::IngestSpans(spans))
            .await
            .map_err(|_| StorageError::CorruptRecord("BatchWriter actor terminated".to_string()))
    }

    /// Queues evaluations for ingestion.
    pub async fn send_evaluations(&self, evaluations: Vec<EvaluationRecord>) -> Result<(), StorageError> {
        self.sender
            .send(WriteCommand::IngestEvaluations(evaluations))
            .await
            .map_err(|_| StorageError::CorruptRecord("BatchWriter actor terminated".to_string()))
    }

    /// Forces an immediate commit of all currently queued items.
    pub async fn flush(&self) -> Result<(), StorageError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(WriteCommand::Flush(tx))
            .await
            .map_err(|_| StorageError::CorruptRecord("BatchWriter actor terminated".to_string()))?;
        let _ = rx.await;
        Ok(())
    }
}
