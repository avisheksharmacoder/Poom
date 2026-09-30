use std::collections::HashMap;
use poom_types::{SpanId, SpanRecord};

/// A node in a reconstructed hierarchical span execution tree.
#[derive(Debug, Clone, PartialEq)]
pub struct SpanNode {
    /// The span execution record.
    pub span: SpanRecord,
    /// Ordered child executions.
    pub children: Vec<SpanNode>,
}

impl SpanNode {
    pub fn new(span: SpanRecord) -> Self {
        Self {
            span,
            children: Vec::new(),
        }
    }

    /// Recursively counts the total number of spans in this subtree.
    pub fn total_nodes(&self) -> usize {
        1 + self.children.iter().map(|c| c.total_nodes()).sum::<usize>()
    }
}

/// Assembles a collection of spans into a hierarchical execution tree starting from the root span.
pub fn assemble_span_tree(spans: Vec<SpanRecord>) -> Option<SpanNode> {
    if spans.is_empty() {
        return None;
    }

    let mut children_map: HashMap<SpanId, Vec<SpanRecord>> = HashMap::new();
    let mut root_span: Option<SpanRecord> = None;

    for span in spans {
        if span.is_root() {
            root_span = Some(span);
        } else if let Some(parent_id) = span.parent_span_id {
            children_map.entry(parent_id).or_default().push(span);
        }
    }

    let root = root_span?;
    Some(build_node(root, &mut children_map))
}

fn build_node(span: SpanRecord, children_map: &mut HashMap<SpanId, Vec<SpanRecord>>) -> SpanNode {
    let mut node = SpanNode::new(span);
    if let Some(children) = children_map.remove(&node.span.span_id) {
        for child in children {
            node.children.push(build_node(child, children_map));
        }
    }
    // Order children chronologically by start timestamp
    node.children.sort_by_key(|c| c.span.start_time_unix_nanos);
    node
}
