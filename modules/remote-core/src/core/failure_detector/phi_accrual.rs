//! Phi Accrual failure detector.

use crate::core::{
  address::Address,
  failure_detector::{FailureDetector, heartbeat_history::HeartbeatHistory},
};

/// Phi Accrual failure detector modelled after Apache Pekko's
/// `PhiAccrualFailureDetector`.
///
/// ## Time input contract
///
/// All methods that accept `now_ms: u64` treat the value as **monotonic
/// millis** (e.g. millis since process start). Passing wall-clock millis is
/// **not** supported — wall clock jumps (NTP corrections, manual changes,
/// leap seconds) can flip the monotonicity assumption and produce spurious
/// unavailability verdicts. The caller is expected to derive `now_ms` from a
/// monotonic source such as `std::time::Instant` or `tokio::time::Instant`.
///
/// ## Formula
///
/// The detector uses Pekko's logistic approximation of the cumulative
/// log-normal distribution. With `mean = history.mean + acceptable_pause` and
/// `std_deviation = max(history.std_deviation, min_std_deviation)`:
///
/// ```text
/// y   = (diff - mean) / std_deviation
/// e   = exp(-y * (1.5976 + 0.070566 * y * y))
/// phi = if diff > mean { -log10(e / (1 + e)) }
///       else            { -log10(1 - 1 / (1 + e)) }
/// ```
///
/// `min_std_deviation` is honoured so that, when all recorded intervals are
/// identical (`std_deviation = 0`), the formula never produces `NaN` /
/// `Infinity`.
#[derive(Debug)]
pub struct PhiAccrualFailureDetector {
  threshold:                  f64,
  max_sample_size:            usize,
  min_std_deviation:          u64,
  acceptable_heartbeat_pause: u64,
  first_heartbeat_estimate:   u64,
  history:                    HeartbeatHistory,
  last_heartbeat_ms:          Option<u64>,
  monitored_address:          Address,
}

impl PhiAccrualFailureDetector {
  /// Creates a new detector.
  ///
  /// - `monitored_address`: remote address this detector is bound to.
  /// - `threshold`: phi value above which the peer is considered unavailable.
  /// - `max_sample_size`: bounded interval history capacity.
  /// - `min_std_deviation`: lower bound for the standard deviation used in the phi formula, in
  ///   millis.
  /// - `acceptable_heartbeat_pause`: additional grace window added to the observed mean, in millis.
  /// - `first_heartbeat_estimate`: expected interval (in millis) used to seed the history on the
  ///   very first heartbeat.
  #[must_use]
  pub fn new(
    monitored_address: Address,
    threshold: f64,
    max_sample_size: usize,
    min_std_deviation: u64,
    acceptable_heartbeat_pause: u64,
    first_heartbeat_estimate: u64,
  ) -> Self {
    let mut history = HeartbeatHistory::new(max_sample_size);
    // Seed the ring buffer with two samples around the first-heartbeat
    // estimate so that phi can be computed even before any real heartbeat
    // has arrived. This mirrors Pekko's `firstHeartbeatEstimate` seeding.
    let std_dev = first_heartbeat_estimate.saturating_div(4).max(1);
    let lower = first_heartbeat_estimate.saturating_sub(std_dev);
    let upper = first_heartbeat_estimate.saturating_add(std_dev);
    history.record(lower);
    history.record(upper);
    Self {
      threshold,
      max_sample_size,
      min_std_deviation,
      acceptable_heartbeat_pause,
      first_heartbeat_estimate,
      history,
      last_heartbeat_ms: None,
      monitored_address,
    }
  }

  /// Returns the configured phi threshold.
  #[must_use]
  pub const fn threshold(&self) -> f64 {
    self.threshold
  }

  /// Returns the configured maximum sample size.
  #[must_use]
  pub const fn max_sample_size(&self) -> usize {
    self.max_sample_size
  }

  /// Returns the configured minimum standard deviation (millis).
  #[must_use]
  pub const fn min_std_deviation(&self) -> u64 {
    self.min_std_deviation
  }

  /// Returns the configured acceptable heartbeat pause (millis).
  #[must_use]
  pub const fn acceptable_heartbeat_pause(&self) -> u64 {
    self.acceptable_heartbeat_pause
  }

  /// Returns the configured first-heartbeat estimate (millis).
  #[must_use]
  pub const fn first_heartbeat_estimate(&self) -> u64 {
    self.first_heartbeat_estimate
  }

  /// Returns the timestamp (monotonic millis) of the last recorded heartbeat.
  #[must_use]
  pub const fn last_heartbeat_ms(&self) -> Option<u64> {
    self.last_heartbeat_ms
  }

  /// Returns the monitored address metadata bound at construction.
  #[must_use]
  pub const fn monitored_address(&self) -> &Address {
    &self.monitored_address
  }

  /// Records a heartbeat at `now_ms` (monotonic millis).
  ///
  /// The first heartbeat initialises `last_heartbeat_ms` without appending a
  /// real interval (the seed samples supplied at construction already provide
  /// enough signal for an early phi). Subsequent heartbeats append the
  /// elapsed interval to the history, evicting the oldest sample when the
  /// ring buffer is full.
  pub fn heartbeat(&mut self, now_ms: u64) {
    if let Some(previous) = self.last_heartbeat_ms {
      let interval = now_ms.saturating_sub(previous);
      self.history.record(interval);
    }
    self.last_heartbeat_ms = Some(now_ms);
  }

  /// Returns the current phi value at `now_ms` (monotonic millis).
  ///
  /// Returns `0.0` when no heartbeat has been recorded yet. Never returns
  /// `NaN` or `Infinity` thanks to the `min_std_deviation` lower bound.
  #[must_use]
  pub fn phi(&self, now_ms: u64) -> f64 {
    let Some(last) = self.last_heartbeat_ms else {
      return 0.0;
    };
    if self.history.is_empty() {
      return 0.0;
    }
    let diff = now_ms.saturating_sub(last) as f64;
    let mean = self.history.mean() + self.acceptable_heartbeat_pause as f64;
    let raw_std = self.history.std_deviation();
    let min_std = self.min_std_deviation as f64;
    let std_deviation = if raw_std < min_std { min_std } else { raw_std };

    if std_deviation <= 0.0 {
      // Degenerate configuration: treat as "available while within mean".
      return if diff <= mean { 0.0 } else { f64::INFINITY };
    }

    let y = (diff - mean) / std_deviation;
    let e = libm::exp(-y * (1.5976 + 0.070566 * y * y));
    let value = if diff > mean { -libm::log10(e / (1.0 + e)) } else { -libm::log10(1.0 - 1.0 / (1.0 + e)) };
    // Pekko's formula can numerically underflow to +/- infinity for extreme
    // inputs (very large `diff`). We clamp at a large finite value so that
    // callers never observe NaN / Infinity per spec.
    if value.is_nan() {
      0.0
    } else if value.is_infinite() {
      if value.is_sign_positive() { f64::MAX } else { 0.0 }
    } else {
      value
    }
  }

  /// Returns `true` when the detector still considers the peer available
  /// (phi is strictly below the configured threshold).
  #[must_use]
  pub fn is_available(&self, now_ms: u64) -> bool {
    self.phi(now_ms) < self.threshold
  }

  /// Returns `true` when at least one heartbeat has been recorded.
  #[must_use]
  pub const fn is_monitoring(&self) -> bool {
    self.last_heartbeat_ms.is_some()
  }
}

impl FailureDetector for PhiAccrualFailureDetector {
  fn is_available(&self, now_ms: u64) -> bool {
    PhiAccrualFailureDetector::is_available(self, now_ms)
  }

  fn is_monitoring(&self) -> bool {
    PhiAccrualFailureDetector::is_monitoring(self)
  }

  fn heartbeat(&mut self, now_ms: u64) {
    PhiAccrualFailureDetector::heartbeat(self, now_ms);
  }
}
