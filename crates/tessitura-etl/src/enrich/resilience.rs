//! Resilience primitives for enrichment stages.

use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};

/// Per-source rate limiter using a token-bucket approach.
///
/// Limits throughput to a configurable number of requests per second by
/// combining a single-permit [`Semaphore`] with a fixed sleep interval.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    interval: Duration,
}

impl RateLimiter {
    /// Creates a new `RateLimiter` that allows at most
    /// `requests_per_second` requests per second.
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(1)),
            interval: Duration::from_millis(1000 / u64::from(requests_per_second)),
        }
    }

    /// Waits until a request slot is available, then holds the slot for
    /// the configured interval to enforce the rate limit.
    pub async fn acquire(&self) {
        // `acquire` only returns `Err` when the semaphore is closed, which
        // we never do, so `expect` is safe here.
        let _permit = self
            .semaphore
            .acquire()
            .await
            .expect("rate-limiter semaphore unexpectedly closed");
        sleep(self.interval).await;
    }
}
