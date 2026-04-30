//! Failure detectors (`no_std`-friendly, time passed as monotonic millis).
//!
//! This module ships:
//!
//! - the [`FailureDetector`] trait and its concrete implementations ([`PhiAccrualFailureDetector`]
//!   modelled after Apache Pekko's `PhiAccrualFailureDetector`, [`DeadlineFailureDetector`] for
//!   deadline-style probes), and
//! - the registry abstractions [`FailureDetectorRegistry`] / [`DefaultFailureDetectorRegistry`]
//!   that compose per-resource detectors.
//!
//! All implementations are pure value types: every operation takes `now_ms: u64`
//! (monotonic millis) as an explicit argument so higher layers stay in control of
//! the time source.

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
