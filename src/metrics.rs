use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

pub struct ServerMetrics {
    pub total_requests: AtomicUsize,
    pub active_connections: AtomicUsize,
    pub not_found_errors: AtomicUsize,
}

impl ServerMetrics {
    pub fn new() -> Self {
        Self {
            total_requests: AtomicUsize::new(0),
            active_connections: AtomicUsize::new(0),
            not_found_errors: AtomicUsize::new(0),
        }
    }
}

pub struct ActiveConnectionGuard<'a> {
    pub counter: &'a AtomicUsize,
}

impl<'a> Drop for ActiveConnectionGuard<'a> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}
