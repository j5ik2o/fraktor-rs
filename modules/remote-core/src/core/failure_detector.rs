//! Failure detectors (`no_std`-friendly, time passed as monotonic millis).
//!
//! Models Apache Pekko's `PhiAccrualFailureDetector` (Scala, ~295 lines). The
//! implementation is a pure value type: every operation takes `now_ms: u64`
//! (monotonic millis) as an explicit argument so the higher layers stay in
//! control of time input.

#[cfg(test)]
mod tests;

mod deadline_failure_detector;
mod heartbeat_history;
mod phi_accrual;

pub use deadline_failure_detector::DeadlineFailureDetector;
pub use heartbeat_history::HeartbeatHistory;
pub use phi_accrual::PhiAccrualFailureDetector;
