//! Phi accrual failure detector for a single monitored resource.

mod config;
#[cfg(test)]
mod tests;

use alloc::collections::VecDeque;

pub use config::PhiFailureDetectorConfig;

use crate::core::failure_detector::failure_detector::FailureDetector;

/// Phi accrual failure detector monitoring a single resource.
///
/// Implements the phi accrual algorithm: the suspicion level grows as the
/// silence interval diverges from the observed heartbeat mean.
pub struct PhiFailureDetector {
  config:         PhiFailureDetectorConfig,
  last_heartbeat: Option<u64>,
  intervals_ms:   VecDeque<u64>,
}

impl PhiFailureDetector {
  /// Creates a detector with the provided configuration.
  #[must_use]
  pub fn new(config: PhiFailureDetectorConfig) -> Self {
    let capacity = config.max_sample_size();
    Self { config, last_heartbeat: None, intervals_ms: VecDeque::with_capacity(capacity) }
  }

  /// Returns the current phi value for the given timestamp.
  #[must_use]
  pub fn phi(&self, now_ms: u64) -> f64 {
    let Some(last) = self.last_heartbeat else {
      return 0.0;
    };
    if self.intervals_ms.is_empty() {
      return 0.0;
    }
    let elapsed = now_ms.saturating_sub(last) as f64;
    let mean = self.intervals_ms.iter().copied().sum::<u64>() as f64 / self.intervals_ms.len() as f64;
    if mean <= 0.0 {
      return 0.0;
    }
    elapsed / mean
  }
}

impl FailureDetector for PhiFailureDetector {
  fn is_available(&self, now_ms: u64) -> bool {
    self.phi(now_ms) < self.config.threshold()
  }

  fn is_monitoring(&self) -> bool {
    self.last_heartbeat.is_some()
  }

  fn heartbeat(&mut self, now_ms: u64) {
    if let Some(previous) = self.last_heartbeat {
      let interval = now_ms.saturating_sub(previous).max(self.config.minimum_interval_ms());
      if self.intervals_ms.len() == self.config.max_sample_size() {
        self.intervals_ms.pop_front();
      }
      self.intervals_ms.push_back(interval);
    }
    self.last_heartbeat = Some(now_ms);
  }
}
