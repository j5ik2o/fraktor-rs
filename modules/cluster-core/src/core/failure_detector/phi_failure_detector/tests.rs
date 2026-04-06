use super::{PhiFailureDetector, PhiFailureDetectorConfig};
use crate::core::failure_detector::FailureDetector;

fn detector() -> PhiFailureDetector {
  PhiFailureDetector::new(PhiFailureDetectorConfig::new(1.5, 4, 1))
}

#[test]
fn should_be_available_when_no_heartbeats_recorded() {
  let det = detector();
  assert!(det.is_available(100));
  assert!(!det.is_monitoring());
}

#[test]
fn should_be_monitoring_after_first_heartbeat() {
  let mut det = detector();
  det.heartbeat(0);
  assert!(det.is_monitoring());
}

#[test]
fn should_be_unavailable_when_phi_exceeds_threshold() {
  let mut det = detector();
  det.heartbeat(0);
  det.heartbeat(10);
  assert!(det.is_available(10));
  assert!(!det.is_available(40));
}

#[test]
fn should_recover_availability_after_new_heartbeat() {
  let mut det = detector();
  det.heartbeat(0);
  det.heartbeat(10);
  assert!(!det.is_available(40));
  det.heartbeat(41);
  assert!(det.is_available(41));
}

#[test]
fn should_return_zero_phi_when_no_heartbeats() {
  let det = detector();
  assert!((det.phi(100) - 0.0).abs() < f64::EPSILON);
}
