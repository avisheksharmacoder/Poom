use redb::{Database, ReadableMultimapTable};

use poom_types::{SpanRecord, TraceId};
use crate::error::StorageError;
use crate::keys::decode_time_index_key;
use crate::schema::*;

/// Retention manager for purging expired spans and compacting storage.
pub struct StoragePruner<'a> {
    db: &'a Database,
}

impl<'a> StoragePruner<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Purges all traces with a start timestamp older than `cutoff_nanos`.
    ///
    /// Executes cascading deletion across:
    /// - `TIME_INDEX`
    /// - `TRACE_SPANS`
    /// - `SPANS`
    /// - `SPAN_CHILDREN`
    /// - `TRACE_EVALUATIONS`
    /// - `EVALUATIONS`
    ///
    /// Returns the number of pruned traces.
    pub fn prune_older_than(&self, cutoff_nanos: u64) -> Result<usize, StorageError> {
        let mut expired_traces: Vec<([u8; 24], TraceId)> = Vec::new();

        // Phase 1: Forward scan TIME_INDEX to locate expired traces
        {
            let read_txn = self.db.begin_read()?;
            let time_index = read_txn.open_table(TIME_INDEX_TABLE)?;

            let mut end_key = [0xFFu8; 24];
            end_key[0..8].copy_from_slice(&cutoff_nanos.to_be_bytes());

            let start_key = [0u8; 24];
            let range = time_index.range::<&[u8; 24]>(&start_key..=&end_key)?;

            for item in range {
                let (k, _) = item?;
                let (timestamp, trace_id) = decode_time_index_key(k.value());
                if timestamp <= cutoff_nanos {
                    expired_traces.push((*k.value(), trace_id));
                }
            }
        }

        if expired_traces.is_empty() {
            return Ok(0);
        }

        // Phase 2: Cascading atomic deletion in a single write transaction
        let write_txn = self.db.begin_write()?;
        {
            let mut time_index = write_txn.open_table(TIME_INDEX_TABLE)?;
            let mut trace_spans = write_txn.open_multimap_table(TRACE_SPANS_TABLE)?;
            let mut spans_table = write_txn.open_table(SPANS_TABLE)?;
            let mut span_children = write_txn.open_multimap_table(SPAN_CHILDREN_TABLE)?;
            let mut trace_evals = write_txn.open_multimap_table(TRACE_EVALUATIONS_TABLE)?;
            let mut evals_table = write_txn.open_table(EVALUATIONS_TABLE)?;

            for (time_key, trace_id) in &expired_traces {
                // Delete from TIME_INDEX
                time_index.remove(time_key)?;

                // Locate and delete all associated spans
                let mut span_ids = Vec::new();
                let iter = trace_spans.get(trace_id.as_bytes())?;
                for span_id_entry in iter {
                    let span_id_bytes = *span_id_entry?.value();
                    span_ids.push(span_id_bytes);
                }

                for span_id_bytes in span_ids {
                    // Try to decode span to clean up SPAN_CHILDREN
                    if let Some(entry) = spans_table.remove(&span_id_bytes)? {
                        if let Ok(span) = postcard::from_bytes::<SpanRecord>(entry.value()) {
                            if let Some(parent_id) = span.parent_span_id {
                                let _ = span_children.remove(parent_id.as_bytes(), span.span_id.as_bytes());
                            }
                        }
                    }
                }

                // Delete all entries from TRACE_SPANS for this trace
                trace_spans.remove_all(trace_id.as_bytes())?;

                // Delete evaluations
                let mut eval_ids = Vec::new();
                let eval_iter = trace_evals.get(trace_id.as_bytes())?;
                for eval_id_entry in eval_iter {
                    eval_ids.push(*eval_id_entry?.value());
                }
                for eval_id_bytes in eval_ids {
                    evals_table.remove(&eval_id_bytes)?;
                }
                trace_evals.remove_all(trace_id.as_bytes())?;
            }
        }
        write_txn.commit()?;

        Ok(expired_traces.len())
    }
}
