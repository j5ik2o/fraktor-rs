use super::{DeadlineFailureDetector, DeadlineFailureDetectorConfig};
use crate::core::failure_detector::FailureDetector;

fn detector(pause_ms: u64, interval_ms: u64) -> DeadlineFailureDetector {
  DeadlineFailureDetector::new(DeadlineFailureDetectorConfig::new(pause_ms, interval_ms))
}

#[test]
fn should_be_available_when_no_heartbeats_recorded() {
  let det = detector(500, 1000);
  assert!(det.is_available(9999));
  assert!(!det.is_monitoring());
}

#[test]
fn should_be_monitoring_after_first_heartbeat() {
  let mut det = detector(500, 1000);
  det.heartbeat(100);
  assert!(det.is_monitoring());
}

#[test]
fn should_be_available_within_deadline() {
  let mut det = detector(500, 1000);
  det.heartbeat(100);
  // deadline = 100 + 1500 = 1600
  assert!(det.is_available(1599));
}

#[test]
fn should_be_unavailable_after_deadline() {
  let mut det = detector(500, 1000);
  det.heartbeat(100);
  // deadline = 100 + 1500 = 1600
  assert!(!det.is_available(1600));
}

#[test]
fn should_recover_after_new_heartbeat() {
  let mut det = detector(500, 1000);
  det.heartbeat(100);
  assert!(!det.is_available(1600));
  det.heartbeat(1600);
  // deadline = 1600 + 1500 = 3100
  assert!(det.is_available(1601));
}

#[test]
#[should_panic(expected = "heartbeat_interval_ms must be > 0")]
fn should_panic_when_interval_is_zero() {
  let _ = DeadlineFailureDetectorConfig::new(500, 0);
}
