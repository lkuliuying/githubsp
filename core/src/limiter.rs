use crate::error::{DownloadError, Result};
use std::time::{Duration, Instant};
use tokio::sync::{watch, Mutex};
use tokio_util::sync::CancellationToken;

pub struct RateLimiter {
    rate: watch::Sender<u32>,
    budget: Mutex<(Instant, f64)>,
}
impl Default for RateLimiter {
    fn default() -> Self {
        let (rate, _) = watch::channel(0);
        Self {
            rate,
            budget: Mutex::new((Instant::now(), 0.0)),
        }
    }
}
impl RateLimiter {
    pub fn set(&self, kib: u32) {
        self.rate.send_replace(kib);
    }
    pub async fn consume(&self, bytes: usize, token: &CancellationToken) -> Result<()> {
        let mut changes = self.rate.subscribe();
        let mut remaining = bytes as f64;
        while remaining > 0.0 {
            if token.is_cancelled() {
                return Err(DownloadError::cancelled());
            }
            let rate = *changes.borrow_and_update() as f64 * 1024.0;
            if rate == 0.0 {
                return Ok(());
            }
            let delay = {
                let mut budget = self.budget.lock().await;
                let capacity = (rate / 10.0).max(1.0);
                budget.1 = (budget.1 + budget.0.elapsed().as_secs_f64() * rate).min(capacity);
                budget.0 = Instant::now();
                let granted = remaining.min(budget.1.floor());
                budget.1 -= granted;
                remaining -= granted;
                Duration::from_secs_f64(
                    ((remaining.min(capacity) - budget.1).max(1.0) / rate).clamp(0.001, 0.1),
                )
            };
            if remaining <= 0.0 {
                break;
            }
            // 限速等待独立于网络读取超时，两个分片共用同一额度。
            tokio::select! { _ = token.cancelled() => return Err(DownloadError::cancelled()), _ = changes.changed() => (), _ = tokio::time::sleep(delay) => () }
        }
        Ok(())
    }
}
