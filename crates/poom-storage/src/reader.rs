use redb::{Database, ReadableTable};

use poom_types::{EvaluationRecord, SpanId, SpanRecord, TraceId};
use crate::error::StorageError;
use crate::keys::{decode_time_index_key, encode_tag_prefix, extract_trace_id_from_tag_key};
use crate::schema::*;
use crate::tree::{assemble_span_tree, SpanNode};

/// Storage reader providing zero-copy snapshot queries over redb B-Trees.
pub struct StorageReader<'a> {
    db: &'a Database,
}

impl<'a> StorageReader<'a> {
    /// Creates a reader bound to the database.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Fetches an individual span record by its SpanId.
    pub fn get_span(&self, span_id: SpanId) -> Result<Option<SpanRecord>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SPANS_TABLE)?;
        if let Some(entry) = table.get(span_id.as_bytes())? {
            let span: SpanRecord = postcard::from_bytes(entry.value())?;
            Ok(Some(span))
        } else {
            Ok(None)
        }
    }

    /// Retrieves all spans associated with a given TraceId.
    pub fn get_trace_spans(&self, trace_id: TraceId) -> Result<Vec<SpanRecord>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let trace_spans = read_txn.open_multimap_table(TRACE_SPANS_TABLE)?;
        let spans_table = read_txn.open_table(SPANS_TABLE)?;

        let mut spans = Vec::new();
        let iter = trace_spans.get(trace_id.as_bytes())?;

        for span_id_entry in iter {
            let entry = span_id_entry?;
            let span_id_bytes = entry.value();
            if let Some(span_entry) = spans_table.get(span_id_bytes)? {
                let span: SpanRecord = postcard::from_bytes(span_entry.value())?;
                spans.push(span);
            }
        }

        // Chronologically order spans by start timestamp
        spans.sort_by_key(|s| s.start_time_unix_nanos);
        Ok(spans)
    }

    /// Reconstructs the full hierarchical execution tree for a trace.
    pub fn get_trace_tree(&self, trace_id: TraceId) -> Result<Option<SpanNode>, StorageError> {
        let spans = self.get_trace_spans(trace_id)?;
        Ok(assemble_span_tree(spans))
    }

    /// Returns recent traces in reverse chronological order (newest first).
    pub fn list_recent_traces(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(u64, TraceId)>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let time_index = read_txn.open_table(TIME_INDEX_TABLE)?;

        let mut traces = Vec::with_capacity(limit);
        let iter = time_index.iter()?;

        for item in iter.rev().skip(offset).take(limit) {
            let (key, _) = item?;
            let (timestamp, trace_id) = decode_time_index_key(key.value());
            traces.push((timestamp, trace_id));
        }

        Ok(traces)
    }

    /// Returns traces whose start timestamp falls within a bounded range `[start_nanos, end_nanos]`.
    pub fn list_traces_in_range(
        &self,
        start_nanos: u64,
        end_nanos: u64,
        limit: usize,
    ) -> Result<Vec<(u64, TraceId)>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let time_index = read_txn.open_table(TIME_INDEX_TABLE)?;

        let mut start_key = [0u8; 24];
        start_key[0..8].copy_from_slice(&start_nanos.to_be_bytes());

        let mut end_key = [0xFFu8; 24];
        end_key[0..8].copy_from_slice(&end_nanos.to_be_bytes());

        let mut traces = Vec::new();
        let range = time_index.range::<&[u8; 24]>(&start_key..=&end_key)?;

        for item in range.take(limit) {
            let (key, _) = item?;
            let (timestamp, trace_id) = decode_time_index_key(key.value());
            traces.push((timestamp, trace_id));
        }

        Ok(traces)
    }

    /// Finds traces matching a specific attribute tag key and value.
    pub fn find_traces_by_tag(
        &self,
        key: &str,
        val: &str,
        limit: usize,
    ) -> Result<Vec<TraceId>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let tag_table = read_txn.open_table(TAG_INDEX_TABLE)?;

        let prefix = encode_tag_prefix(key, val);
        let mut traces = Vec::new();
        let iter = tag_table.range::<&[u8]>(prefix.as_slice()..)?;

        for item in iter {
            let (k, _) = item?;
            let key_bytes = k.value();

            if !key_bytes.starts_with(prefix.as_slice()) {
                break;
            }

            if let Some(trace_id) = extract_trace_id_from_tag_key(key_bytes) {
                traces.push(trace_id);
                if traces.len() >= limit {
                    break;
                }
            }
        }

        Ok(traces)
    }

    /// Retrieves all evaluations linked to a trace.
    pub fn get_trace_evaluations(
        &self,
        trace_id: TraceId,
    ) -> Result<Vec<EvaluationRecord>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let trace_evals = match read_txn.open_multimap_table(TRACE_EVALUATIONS_TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let evals_table = match read_txn.open_table(EVALUATIONS_TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };

        let mut evaluations = Vec::new();
        let iter = trace_evals.get(trace_id.as_bytes())?;

        for eval_id_entry in iter {
            let entry = eval_id_entry?;
            let eval_id_bytes = entry.value();
            if let Some(eval_entry) = evals_table.get(eval_id_bytes)? {
                let eval: EvaluationRecord = postcard::from_bytes(eval_entry.value())?;
                evaluations.push(eval);
            }
        }

        Ok(evaluations)
    }
}
