use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

/// Runtime telemetry and operational metrics for the daemon.
pub struct DaemonTelemetry {
    start_time: Instant,
    active_connections: AtomicUsize,
    total_spans_ingested: AtomicU64,
    total_evaluations_ingested: AtomicU64,
}

impl Default for DaemonTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl DaemonTelemetry {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            active_connections: AtomicUsize::new(0),
            total_spans_ingested: AtomicU64::new(0),
            total_evaluations_ingested: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn inc_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn dec_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn add_spans(&self, count: usize) {
        self.total_spans_ingested.fetch_add(count as u64, Ordering::Relaxed);
    }

    #[inline]
    pub fn add_evaluations(&self, count: usize) {
        self.total_evaluations_ingested.fetch_add(count as u64, Ordering::Relaxed);
    }

    pub fn active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }

    pub fn total_spans(&self) -> u64 {
        self.total_spans_ingested.load(Ordering::Relaxed)
    }

    pub fn total_evaluations(&self) -> u64 {
        self.total_evaluations_ingested.load(Ordering::Relaxed)
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}
