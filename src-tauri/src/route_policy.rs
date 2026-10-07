use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(crate) struct RoutePolicy {
    pub recovery_budget: Duration,
    pub backoff_base: Duration,
    pub backoff_cap: Duration,
    pub tick: Duration,
    pub slow_window: Duration,
    pub probe_interval: Duration,
    pub dismiss_interval: Duration,
}

impl Default for RoutePolicy {
    fn default() -> Self {
        Self {
            recovery_budget: Duration::from_secs(300),
            backoff_base: Duration::from_secs(10),
            backoff_cap: Duration::from_secs(60),
            tick: Duration::from_secs(1),
            slow_window: Duration::from_secs(30),
            probe_interval: Duration::from_secs(120),
            dismiss_interval: Duration::from_secs(600),
        }
    }
}

impl RoutePolicy {
    pub fn backoff(&self, round: u32, jitter: u8) -> Duration {
        let base = self
            .backoff_base
            .saturating_mul(1 << round.min(3))
            .min(self.backoff_cap);
        // 只增加少量抖动，不缩短服务端要求的等待时间。
        base.saturating_add(base.mul_f64(f64::from(jitter % 21) / 100.0))
            .min(self.backoff_cap)
    }
}

pub(crate) struct RecoveryClock {
    pub remaining: Duration,
    pub wait: Duration,
    pub round: u32,
    last_accounted: Instant,
}

impl RecoveryClock {
    pub fn new(now: Instant, policy: &RoutePolicy, jitter: u8) -> Self {
        Self {
            remaining: policy.recovery_budget,
            wait: policy.backoff(0, jitter),
            round: 0,
            last_accounted: now,
        }
    }

    pub fn account(&mut self, now: Instant, occupied_by_other: bool) {
        let elapsed = now.saturating_duration_since(self.last_accounted);
        self.last_accounted = now;
        if !occupied_by_other {
            self.remaining = self.remaining.saturating_sub(elapsed);
            self.wait = self.wait.saturating_sub(elapsed);
        }
    }

    pub fn failed_round(&mut self, policy: &RoutePolicy, jitter: u8) {
        self.round = self.round.saturating_add(1);
        self.wait = policy.backoff(self.round, jitter);
    }
}

pub(crate) struct SpeedWindow {
    samples: VecDeque<(Instant, u64)>,
    retention: Duration,
}

impl SpeedWindow {
    pub fn new(now: Instant, bytes: u64, retention: Duration) -> Self {
        Self {
            samples: VecDeque::from([(now, bytes)]),
            retention: retention.max(Duration::from_secs(5)),
        }
    }

    pub fn record(&mut self, now: Instant, bytes: u64) {
        if self.samples.back().is_some_and(|(_, old)| bytes < *old) {
            self.samples.clear();
        }
        if self.samples.back().is_some_and(|(time, _)| *time == now) {
            self.samples.pop_back();
        }
        self.samples.push_back((now, bytes));
        // 多保留窗口前的一个样本，用于低频进度下的线性插值。
        while self.samples.len() > 2
            && now.saturating_duration_since(self.samples[1].0) >= self.retention
        {
            self.samples.pop_front();
        }
    }

    pub fn average(&self, now: Instant, window: Duration, full: bool) -> Option<f64> {
        let &(first_time, first_bytes) = self.samples.front()?;
        let &(last_time, last_bytes) = self.samples.back()?;
        let elapsed = now.saturating_duration_since(first_time);
        if full && elapsed < window {
            return None;
        }
        let seconds = elapsed.min(window).as_secs_f64();
        if seconds <= 0.0 {
            return None;
        }
        let cutoff = now
            .checked_sub(window)
            .unwrap_or(first_time)
            .max(first_time);
        let mut start_bytes = first_bytes as f64;
        for pair in self.samples.iter().zip(self.samples.iter().skip(1)) {
            let (&(left_time, left), &(right_time, right)) = pair;
            if left_time <= cutoff && cutoff <= right_time {
                let span = right_time
                    .saturating_duration_since(left_time)
                    .as_secs_f64();
                let fraction = if span > 0.0 {
                    cutoff.saturating_duration_since(left_time).as_secs_f64() / span
                } else {
                    1.0
                };
                start_bytes = left as f64 + right.saturating_sub(left) as f64 * fraction;
                break;
            }
        }
        if cutoff >= last_time {
            start_bytes = last_bytes as f64;
        }
        Some(((last_bytes as f64 - start_bytes) / seconds).max(0.0))
    }
}

pub(crate) fn low_speed(
    total: Option<u64>,
    downloaded: u64,
    speed: Option<f64>,
    baseline: f64,
    limited: bool,
) -> bool {
    let Some(speed) = speed.filter(|speed| speed.is_finite() && *speed > 0.0) else {
        return false;
    };
    let Some(total) = total.filter(|total| *total > downloaded) else {
        return false;
    };
    !limited
        && (total - downloaded) as f64 / speed >= 60.0
        && (speed < 256_000.0 || baseline.is_finite() && speed < baseline * 0.25)
}

pub(crate) fn savings(
    total: u64,
    downloaded: u64,
    current: f64,
    candidate: f64,
) -> Option<(u64, u64)> {
    if !current.is_finite()
        || !candidate.is_finite()
        || current <= 0.0
        || candidate <= 0.0
        || downloaded >= total
    {
        return None;
    }
    let old = (total - downloaded) as f64 / current;
    let new = total as f64 / candidate + 10.0;
    (old >= 60.0 && new <= old * 0.7 && old - new >= 30.0)
        .then_some((old.ceil() as u64, new.ceil() as u64))
}

pub(crate) fn millis(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_budget_counts_wait_and_probes_but_freezes_for_other_downloads() {
        let now = Instant::now();
        let policy = RoutePolicy::default();
        let mut clock = RecoveryClock::new(now, &policy, 0);
        clock.account(now + Duration::from_secs(4), false);
        assert_eq!(clock.remaining, Duration::from_secs(296));
        assert_eq!(clock.wait, Duration::from_secs(6));
        clock.account(now + Duration::from_secs(604), true);
        assert_eq!(clock.remaining, Duration::from_secs(296));
        assert_eq!(clock.wait, Duration::from_secs(6));
        clock.account(now + Duration::from_secs(610), false);
        assert!(clock.wait.is_zero());
        clock.failed_round(&policy, 0);
        assert_eq!(clock.wait, Duration::from_secs(20));
        clock.account(now + Duration::from_secs(900), false);
        assert!(clock.remaining.is_zero());
        for (round, seconds) in [(0, 10), (1, 20), (2, 40), (3, 60), (99, 60)] {
            assert_eq!(policy.backoff(round, 0), Duration::from_secs(seconds));
            assert!(policy.backoff(round, 20) >= Duration::from_secs(seconds));
            assert!(policy.backoff(round, 20) <= Duration::from_secs(60));
        }
    }

    #[test]
    fn speed_window_requires_sustained_samples_decays_and_resets_after_restart() {
        let now = Instant::now();
        let mut window = SpeedWindow::new(now, 0, Duration::from_secs(30));
        for second in 1..=30 {
            window.record(now + Duration::from_secs(second), second * 100_000);
        }
        assert_eq!(
            window.average(now + Duration::from_secs(29), Duration::from_secs(30), true),
            None
        );
        assert_eq!(
            window.average(now + Duration::from_secs(30), Duration::from_secs(30), true),
            Some(100_000.0)
        );
        assert_eq!(
            window.average(now + Duration::from_secs(35), Duration::from_secs(5), false),
            Some(0.0)
        );
        window.record(now + Duration::from_secs(36), 0);
        assert_eq!(
            window.average(now + Duration::from_secs(36), Duration::from_secs(30), true),
            None
        );
    }

    #[test]
    fn suggestions_exclude_caps_short_remaining_unknown_sizes_and_bad_estimates() {
        assert!(low_speed(
            Some(100_000_000),
            0,
            Some(100_000.0),
            1_000_000.0,
            false
        ));
        assert!(low_speed(
            Some(1_000_000_000),
            0,
            Some(1_000_000.0),
            8_000_000.0,
            false
        ));
        assert!(!low_speed(
            Some(100_000_000),
            0,
            Some(100_000.0),
            1_000_000.0,
            true
        ));
        assert!(!low_speed(None, 0, Some(100_000.0), 1_000_000.0, false));
        assert!(!low_speed(
            Some(5_000_000),
            0,
            Some(100_000.0),
            1_000_000.0,
            false
        ));
        for speed in [None, Some(0.0), Some(f64::NAN), Some(f64::INFINITY)] {
            assert!(!low_speed(Some(100_000_000), 0, speed, 1_000_000.0, false));
        }
        assert_eq!(
            savings(100_000_000, 20_000_000, 100_000.0, 1_000_000.0),
            Some((800, 110))
        );
        assert!(savings(100_000_000, 95_000_000, 100_000.0, 1_000_000.0).is_none());
        assert!(savings(100_000_000, 60_000_000, 100_000.0, 200_000.0).is_none());
        assert!(savings(100_000_000, 0, 100_000.0, f64::NAN).is_none());
    }
}
