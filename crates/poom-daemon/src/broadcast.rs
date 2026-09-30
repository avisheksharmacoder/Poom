use tokio::sync::broadcast;
use poom_types::SpanRecord;

/// Real-time live span event broadcaster for UI and telemetry subscribers.
#[derive(Clone)]
pub struct EventBroadcaster {
    sender: broadcast::Sender<SpanRecord>,
}

impl EventBroadcaster {
    /// Constructs a broadcaster with a bounded ring buffer capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribes to the live span stream.
    pub fn subscribe(&self) -> broadcast::Receiver<SpanRecord> {
        self.sender.subscribe()
    }

    /// Dispatches an enriched span record to all active subscribers.
    ///
    /// Non-blocking: If a subscriber is lagged, it will miss events rather than
    /// stalling the ingestion pipeline.
    pub fn broadcast(&self, span: SpanRecord) -> usize {
        self.sender.send(span).unwrap_or(0)
    }

    /// Returns the number of active subscribers.
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}
