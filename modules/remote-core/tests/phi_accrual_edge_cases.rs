//! Edge-case integration tests for the new
//! [`fraktor_remote_core_rs::domain::failure_detector::PhiAccrualFailureDetector`].
//!
//! Replaces the legacy `modules/remote/tests/phi_failure_detector_edge_cases.rs`
//! for the redesigned `remote-core` API. The legacy tests targeted the old
//! `PhiFailureDetector` (a simple `elapsed/mean` heuristic) and a now-removed
//! registry abstraction; the new core ships a Pekko-compatible Phi Accrual
//! algorithm that takes monotonic millis as an explicit `now_ms` argument and
//! does not own a per-node registry (the watcher state owns the per-node
//! lookup). This integration test focuses on the edge cases that exercise the
//! external contract of [`PhiAccrualFailureDetector`].

use fraktor_remote_core_rs::domain::failure_detector::PhiAccrualFailureDetector;

fn detector_with(threshold: f64) -> PhiAccrualFailureDetector {
  PhiAccrualFailureDetector::new(threshold, 100, 10, 0, 100)
}

#[test]
fn detector_is_available_without_heartbeats() {
  let detector = detector_with(8.0);
  assert!(!detector.is_monitoring());
  assert!(detector.is_available(0));
  assert!(detector.is_available(10_000));
}

#[test]
fn detector_becomes_unavailable_after_long_silence() {
  let mut detector = detector_with(5.0);
  // Establish a steady cadence of heartbeats every 100 ms.
  let mut t: u64 = 0;
  for _ in 0..50 {
    detector.heartbeat(t);
    t += 100;
  }
  // After a long silence the peer should be flagged unavailable.
  assert!(!detector.is_available(t + 60_000));
}

#[test]
fn detector_recovers_after_fresh_heartbeat() {
  let mut detector = detector_with(5.0);
  let mut t: u64 = 0;
  for _ in 0..20 {
    detector.heartbeat(t);
    t += 100;
  }
  let last = t - 100;
  assert!(!detector.is_available(last + 60_000));

  // A fresh heartbeat should reset the detector and bring it back to
  // available immediately.
  detector.heartbeat(last + 60_001);
  assert!(detector.is_available(last + 60_001));
}

#[test]
fn detector_handles_constant_intervals_without_diverging() {
  // All heartbeats arrive exactly every 100 ms → standard deviation 0,
  // which would cause naive phi formulas to produce NaN / Infinity.
  let mut detector = PhiAccrualFailureDetector::new(8.0, 100, 0, 0, 100);
  let mut t: u64 = 0;
  for _ in 0..30 {
    detector.heartbeat(t);
    t += 100;
  }
  let phi_now = detector.phi(t);
  assert!(phi_now.is_finite(), "phi must remain finite even when std_dev = 0");
}

#[test]
fn detector_acceptable_pause_widens_availability_window() {
  let mut with_pause = PhiAccrualFailureDetector::new(8.0, 100, 10, 2_000, 100);
  let mut no_pause = PhiAccrualFailureDetector::new(8.0, 100, 10, 0, 100);
  let mut t: u64 = 0;
  for _ in 0..50 {
    with_pause.heartbeat(t);
    no_pause.heartbeat(t);
    t += 100;
  }
  let last = t - 100;
  // 1.2 s after the last heartbeat the with_pause detector still has
  // headroom inside its 2 s grace window.
  assert!(with_pause.is_available(last + 1_200));
  assert!(with_pause.phi(last + 1_200) < no_pause.phi(last + 1_200));
}

#[test]
fn detector_max_sample_size_is_enforced_indirectly() {
  // The max_sample_size guard prevents the underlying ring buffer from
  // growing without bound. We cannot inspect history directly, but we can
  // verify that recording 200 heartbeats does not push phi into the
  // unavailable region.
  let mut detector = PhiAccrualFailureDetector::new(8.0, 10, 10, 0, 100);
  let mut t: u64 = 0;
  detector.heartbeat(t);
  for _ in 0..200 {
    t += 100;
    detector.heartbeat(t);
  }
  assert!(detector.phi(t) < 1.0);
  assert!(detector.is_available(t));
}
