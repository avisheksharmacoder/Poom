use poom_storage::SpanNode;
use poom_types::SpanRecord;

pub const ROW_HEIGHT: f32 = 32.0;
pub const HEADER_HEIGHT: f32 = 28.0;
pub const TREE_LABEL_WIDTH: f32 = 240.0;
pub const MIN_BAR_WIDTH: f32 = 5.0;

/// A flattened span entry prepared for immediate canvas row layout.
#[derive(Debug, Clone)]
pub struct SpanLayoutItem {
    pub span: SpanRecord,
    pub depth: usize,
    pub row_index: usize,
}

/// Global timing bounds across an entire trace tree.
#[derive(Debug, Clone, Copy)]
pub struct TraceBounds {
    pub start_unix_nanos: u64,
    pub end_unix_nanos: u64,
    pub total_duration_nanos: u64,
}

impl TraceBounds {
    /// Computes absolute start and end times across a flattened list of spans.
    pub fn compute(items: &[SpanLayoutItem]) -> Option<Self> {
        if items.is_empty() {
            return None;
        }

        let mut start_nanos = u64::MAX;
        let mut end_nanos = 0u64;

        for item in items {
            let s_start = item.span.start_time_unix_nanos;
            let s_end = item.span.end_time_unix_nanos.unwrap_or(s_start);

            if s_start < start_nanos {
                start_nanos = s_start;
            }
            if s_end > end_nanos {
                end_nanos = s_end;
            }
        }

        let total_duration_nanos = end_nanos.saturating_sub(start_nanos).max(1);

        Some(Self {
            start_unix_nanos: start_nanos,
            end_unix_nanos: end_nanos,
            total_duration_nanos,
        })
    }
}

/// Recursively flattens a SpanNode hierarchy into depth-first row items.
pub fn flatten_span_tree(root: &SpanNode) -> Vec<SpanLayoutItem> {
    let mut items = Vec::new();
    flatten_recursive(root, 0, &mut items);
    items
}

fn flatten_recursive(node: &SpanNode, depth: usize, out: &mut Vec<SpanLayoutItem>) {
    let row_index = out.len();
    out.push(SpanLayoutItem {
        span: node.span.clone(),
        depth,
        row_index,
    });

    for child in &node.children {
        flatten_recursive(child, depth + 1, out);
    }
}

/// Calculates the exact screen pixel bounding box for a span's duration bar.
pub fn calculate_bar_geometry(
    item: &SpanLayoutItem,
    bounds: &TraceBounds,
    canvas_width: f32,
    pan_x: f32,
    pan_y: f32,
    zoom: f32,
) -> (f32, f32, f32, f32) {
    let timeline_width = (canvas_width - TREE_LABEL_WIDTH).max(100.0);

    let offset_nanos = item.span.start_time_unix_nanos.saturating_sub(bounds.start_unix_nanos);
    let dur_nanos = item.span.duration_nanos().unwrap_or(1).max(1);

    let offset_ratio = offset_nanos as f32 / bounds.total_duration_nanos as f32;
    let dur_ratio = dur_nanos as f32 / bounds.total_duration_nanos as f32;

    let bar_x = TREE_LABEL_WIDTH + (offset_ratio * timeline_width * zoom) + pan_x;
    let bar_w = (dur_ratio * timeline_width * zoom).max(MIN_BAR_WIDTH);

    let bar_y = HEADER_HEIGHT + (item.row_index as f32 * ROW_HEIGHT) + pan_y + 3.0;
    let bar_h = ROW_HEIGHT - 6.0;

    (bar_x, bar_y, bar_w, bar_h)
}
