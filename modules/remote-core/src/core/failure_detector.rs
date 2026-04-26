//! Failure detectors (`no_std`-friendly, time passed as monotonic millis).
//!
//! Models Apache Pekko's `PhiAccrualFailureDetector` (Scala, ~295 lines). The
//! implementation is a pure value type: every operation takes `now_ms: u64`
//! (monotonic millis) as an explicit argument so the higher layers stay in
//! control of time input.

#[cfg(test)]
mod tests;

mod deadline_failure_detector;
mod default_failure_detector_registry;
mod failure_detector_registry;
mod heartbeat_history;
mod phi_accrual;

pub use deadline_failure_detector::DeadlineFailureDetector;
pub use default_failure_detector_registry::DefaultFailureDetectorRegistry;
pub use failure_detector_registry::FailureDetectorRegistry;
pub use heartbeat_history::HeartbeatHistory;
pub use phi_accrual::PhiAccrualFailureDetector;

/// Tracks availability for one resource through heartbeat observations.
pub trait FailureDetector {
  /// Returns `true` when the monitored resource is considered available.
  #[must_use]
  fn is_available(&self, now_ms: u64) -> bool;

  /// Returns `true` after at least one heartbeat has been recorded.
  #[must_use]
  fn is_monitoring(&self) -> bool;

  /// Records a heartbeat arrival at monotonic millis.
  fn heartbeat(&mut self, now_ms: u64);
}
