//! Bounded ring buffer of heartbeat intervals (in millis).

use alloc::collections::VecDeque;

/// Bounded ring buffer recording recent heartbeat intervals (in millis).
#[derive(Clone, Debug)]
pub struct HeartbeatHistory {
  max_sample_size: usize,
  intervals:       VecDeque<u64>,
}

impl HeartbeatHistory {
  /// Creates a new [`HeartbeatHistory`] with the given capacity.
  #[must_use]
  pub fn new(max_sample_size: usize) -> Self {
    Self { max_sample_size, intervals: VecDeque::with_capacity(max_sample_size) }
  }

  /// Returns the number of recorded intervals.
  #[must_use]
  pub fn len(&self) -> usize {
    self.intervals.len()
  }

  /// Returns `true` when no interval has been recorded yet.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.intervals.is_empty()
  }

  /// Returns the configured maximum number of samples retained.
  #[must_use]
  pub const fn max_sample_size(&self) -> usize {
    self.max_sample_size
  }

  /// Appends `interval` to the history, evicting the oldest sample when the
  /// ring buffer is full.
  pub fn record(&mut self, interval: u64) {
    if self.intervals.len() == self.max_sample_size {
      self.intervals.pop_front();
    }
    self.intervals.push_back(interval);
  }

  /// Returns the arithmetic mean of the recorded intervals, or `0.0` if empty.
  #[must_use]
  pub fn mean(&self) -> f64 {
    if self.intervals.is_empty() {
      return 0.0;
    }
    let sum: u128 = self.intervals.iter().map(|&v| v as u128).sum();
    (sum as f64) / (self.intervals.len() as f64)
  }

  /// Returns the population standard deviation of the recorded intervals.
  ///
  /// Returns `0.0` when fewer than two samples have been recorded.
  #[must_use]
  pub fn std_deviation(&self) -> f64 {
    if self.intervals.len() < 2 {
      return 0.0;
    }
    let mean = self.mean();
    let variance_sum: f64 = self
      .intervals
      .iter()
      .map(|&v| {
        let d = v as f64 - mean;
        d * d
      })
      .sum();
    let variance = variance_sum / (self.intervals.len() as f64);
    libm::sqrt(variance)
  }
}
