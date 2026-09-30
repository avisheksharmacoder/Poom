use redb::{MultimapTableDefinition, TableDefinition};

/// Primary span storage: SpanId (16 bytes) -> Postcard-serialized SpanRecord bytes.
pub const SPANS_TABLE: TableDefinition<&[u8; 16], &[u8]> = TableDefinition::new("spans");

/// Chronological index: (Timestamp_BE (8B) + TraceId (16B)) -> ().
/// Big-endian nanoseconds ensure natural B-Tree chronological sort order.
pub const TIME_INDEX_TABLE: TableDefinition<&[u8; 24], ()> = TableDefinition::new("time_index");

/// Multimap mapping TraceId (16B) -> SpanId (16B) for instant full-trace extraction.
pub const TRACE_SPANS_TABLE: MultimapTableDefinition<&[u8; 16], &[u8; 16]> =
    MultimapTableDefinition::new("trace_spans");

/// Multimap mapping ParentSpanId (16B) -> ChildSpanId (16B) for recursive execution tree reconstruction.
pub const SPAN_CHILDREN_TABLE: MultimapTableDefinition<&[u8; 16], &[u8; 16]> =
    MultimapTableDefinition::new("span_children");

/// Composite index for tag-based filtering:
/// [tag_key_len: u16][tag_key][tag_val_len: u16][tag_val][trace_id: 16B] -> ().
pub const TAG_INDEX_TABLE: TableDefinition<&[u8], ()> = TableDefinition::new("tag_index");

/// Primary evaluation storage: EvaluationId (16B) -> Postcard-serialized EvaluationRecord bytes.
pub const EVALUATIONS_TABLE: TableDefinition<&[u8; 16], &[u8]> =
    TableDefinition::new("evaluations");

/// Multimap mapping TraceId (16B) -> EvaluationId (16B).
pub const TRACE_EVALUATIONS_TABLE: MultimapTableDefinition<&[u8; 16], &[u8; 16]> =
    MultimapTableDefinition::new("trace_evaluations");
