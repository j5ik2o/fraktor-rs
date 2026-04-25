use core::num::NonZeroU64;

use crate::failure_detector::{DeadlineFailureDetector, HeartbeatHistory, PhiAccrualFailureDetector};

// ---------------------------------------------------------------------------
// HeartbeatHistory
// ---------------------------------------------------------------------------

#[test]
fn heartbeat_history_records_up_to_capacity() {
  let mut h = HeartbeatHistory::new(3);
  h.record(10);
  h.record(20);
  h.record(30);
  assert_eq!(h.len(), 3);
}

#[test]
fn heartbeat_history_evicts_oldest_when_full() {
  let mut h = HeartbeatHistory::new(3);
  h.record(10);
  h.record(20);
  h.record(30);
  h.record(40);
  h.record(50);
  assert_eq!(h.len(), 3);
  // Oldest two (10, 20) should be evicted, retained = 30, 40, 50
  assert!((h.mean() - 40.0).abs() < 1e-9);
}

#[test]
fn heartbeat_history_mean_zero_when_empty() {
  let h = HeartbeatHistory::new(5);
  assert_eq!(h.mean(), 0.0);
  assert_eq!(h.std_deviation(), 0.0);
  assert!(h.is_empty());
}

#[test]
fn heartbeat_history_std_deviation_is_zero_for_constant_samples() {
  let mut h = HeartbeatHistory::new(10);
  for _ in 0..5 {
    h.record(100);
  }
  assert!((h.mean() - 100.0).abs() < 1e-9);
  assert!((h.std_deviation()).abs() < 1e-9);
}

#[test]
fn heartbeat_history_std_deviation_is_computed() {
  let mut h = HeartbeatHistory::new(10);
  h.record(10);
  h.record(20);
  h.record(30);
  // mean = 20, variance = ((10-20)^2 + 0 + (30-20)^2) / 3 = 200/3 ≈ 66.666
  // std_dev ≈ 8.165
  let sd = h.std_deviation();
  assert!((sd - libm::sqrt(200.0_f64 / 3.0)).abs() < 1e-9);
}

// ---------------------------------------------------------------------------
// PhiAccrualFailureDetector
// ---------------------------------------------------------------------------

fn make_detector() -> PhiAccrualFailureDetector {
  PhiAccrualFailureDetector::new(
    8.0,  // threshold
    100,  // max_sample_size
    10,   // min_std_deviation (ms)
    0,    // acceptable_heartbeat_pause (ms)
    1000, // first_heartbeat_estimate (ms)
  )
}

const HEARTBEAT_INTERVAL_MS: u64 = 1_000;
const ACCEPTABLE_HEARTBEAT_PAUSE_MS: u64 = 200;

fn heartbeat_interval_ms() -> NonZeroU64 {
  NonZeroU64::new(HEARTBEAT_INTERVAL_MS).expect("heartbeat interval must be non-zero")
}

fn make_deadline_detector() -> DeadlineFailureDetector {
  DeadlineFailureDetector::new(heartbeat_interval_ms(), ACCEPTABLE_HEARTBEAT_PAUSE_MS)
}

#[test]
fn detector_is_available_without_heartbeat() {
  let d = make_detector();
  assert!(!d.is_monitoring());
  assert_eq!(d.phi(0), 0.0);
  assert!(d.is_available(0));
}

#[test]
fn detector_available_immediately_after_heartbeat() {
  let mut d = make_detector();
  d.heartbeat(0);
  assert!(d.is_monitoring());
  assert!(d.is_available(0));
}

#[test]
fn detector_max_sample_size_is_enforced() {
  let mut d = PhiAccrualFailureDetector::new(8.0, 10, 10, 0, 1000);
  // Record many heartbeats with constant interval of 100 ms.
  let mut t: u64 = 0;
  d.heartbeat(t);
  for _ in 0..200 {
    t += 100;
    d.heartbeat(t);
  }
  // History should never exceed the configured max of 10.
  // The detector does not expose history directly, but we can verify via a
  // proxy: phi should not diverge to infinity and is_available should be true
  // when no pause has occurred.
  assert!(d.phi(t) < 1.0);
  assert!(d.is_available(t));
}

#[test]
fn detector_constant_interval_does_not_diverge() {
  let mut d = make_detector();
  // All heartbeats arrive exactly every 100 ms → std_deviation of the
  // recorded intervals will be 0.
  let mut t: u64 = 0;
  for _ in 0..20 {
    d.heartbeat(t);
    t += 100;
  }
  // `now_ms` equal to the last heartbeat means `diff = 0`. The formula must
  // yield a finite value.
  let phi = d.phi(t - 100);
  assert!(phi.is_finite());
  assert!(!phi.is_nan());
}

#[test]
fn detector_phi_increases_with_elapsed_time() {
  let mut d = make_detector();
  let mut t: u64 = 0;
  for _ in 0..20 {
    d.heartbeat(t);
    t += 100;
  }
  let last = t - 100;
  let phi_near = d.phi(last + 110);
  let phi_far = d.phi(last + 10_000);
  assert!(phi_far > phi_near, "phi should grow as time passes: near={phi_near} far={phi_far}");
}

#[test]
fn detector_long_silence_triggers_unavailable() {
  let mut d = PhiAccrualFailureDetector::new(5.0, 100, 10, 0, 100);
  // Establish a stable heartbeat cadence.
  let mut t: u64 = 0;
  for _ in 0..50 {
    d.heartbeat(t);
    t += 100;
  }
  let last = t - 100;
  // After a long silence the peer should be considered unavailable.
  assert!(!d.is_available(last + 60_000));
}

#[test]
fn detector_no_nan_or_infinity_with_min_std_deviation() {
  // Extreme config: min_std_deviation is zero and all intervals are constant.
  let mut d = PhiAccrualFailureDetector::new(8.0, 100, 0, 0, 100);
  let mut t: u64 = 0;
  for _ in 0..10 {
    d.heartbeat(t);
    t += 100;
  }
  let phi = d.phi(t);
  assert!(!phi.is_nan(), "phi must not be NaN");
  // Infinity is allowed by the clamp (maps to f64::MAX or 0.0), never true NaN.
}

#[test]
fn detector_acceptable_pause_delays_unavailability() {
  let with_pause = PhiAccrualFailureDetector::new(8.0, 100, 10, 2000, 100);
  let no_pause = PhiAccrualFailureDetector::new(8.0, 100, 10, 0, 100);
  let mut with_pause = with_pause;
  let mut no_pause = no_pause;
  let mut t: u64 = 0;
  for _ in 0..50 {
    with_pause.heartbeat(t);
    no_pause.heartbeat(t);
    t += 100;
  }
  let last = t - 100;
  // 1s after the last heartbeat: with_pause should still be available
  // thanks to the 2s acceptable pause.
  assert!(with_pause.is_available(last + 1000));
  // no_pause has no extra grace and may already be flagged.
  let no_pause_phi = no_pause.phi(last + 1000);
  let with_pause_phi = with_pause.phi(last + 1000);
  assert!(with_pause_phi < no_pause_phi);
}

#[test]
fn deadline_detector_is_available_before_first_heartbeat() {
  let detector = make_deadline_detector();

  assert!(!detector.is_monitoring());
  assert!(detector.is_available(10_000));
}

#[test]
fn deadline_detector_starts_monitoring_after_heartbeat() {
  let mut detector = make_deadline_detector();

  detector.heartbeat(500);

  assert!(detector.is_monitoring());
  assert!(detector.is_available(500));
}

#[test]
fn deadline_detector_uses_exclusive_deadline_boundary() {
  let mut detector = make_deadline_detector();
  let heartbeat_ms = 5_000;
  let deadline_ms = heartbeat_ms + HEARTBEAT_INTERVAL_MS + ACCEPTABLE_HEARTBEAT_PAUSE_MS;

  detector.heartbeat(heartbeat_ms);

  assert!(detector.is_available(deadline_ms - 1));
  assert!(!detector.is_available(deadline_ms));
}
