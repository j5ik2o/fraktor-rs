#![cfg(feature = "test-support")]

use fraktor_remote_rs::core::{PhiFailureDetector, PhiFailureDetectorConfig, PhiFailureDetectorEffect};

fn detector_with(threshold: f64, max_sample_size: usize, minimum_interval_ms: u64) -> PhiFailureDetector {
  PhiFailureDetector::new(PhiFailureDetectorConfig::new(threshold, max_sample_size, minimum_interval_ms))
}

#[test]
fn poll_without_heartbeats_returns_no_effects() {
  let mut detector = detector_with(1.5, 8, 1);
  assert!(detector.poll(100).is_empty());
}

#[test]
fn suspect_is_emitted_only_once_until_new_heartbeat_arrives() {
  let mut detector = detector_with(1.5, 8, 1);
  detector.record_heartbeat("node-a:4100", 0);
  detector.record_heartbeat("node-a:4100", 10);

  let first = detector.poll(40);
  assert!(
    matches!(first.as_slice(), [PhiFailureDetectorEffect::Suspect { authority, .. }] if authority == "node-a:4100")
  );

  let second = detector.poll(100);
  assert!(second.is_empty(), "suspect event should not be emitted repeatedly");
}

#[test]
fn reachable_is_emitted_only_for_first_recovery_heartbeat() {
  let mut detector = detector_with(1.5, 8, 1);
  detector.record_heartbeat("node-a:4200", 0);
  detector.record_heartbeat("node-a:4200", 10);
  let suspect = detector.poll(40);
  assert!(matches!(suspect.as_slice(), [PhiFailureDetectorEffect::Suspect { .. }]));

  let first_recovery = detector.record_heartbeat("node-a:4200", 41);
  assert!(
    matches!(
      first_recovery,
      Some(PhiFailureDetectorEffect::Reachable { authority }) if authority == "node-a:4200"
    ),
    "first heartbeat after suspect should emit reachable"
  );

  let second_recovery = detector.record_heartbeat("node-a:4200", 42);
  assert!(second_recovery.is_none(), "steady-state heartbeats should not emit reachable");
}

#[test]
fn minimum_interval_clamps_fast_heartbeats() {
  let mut detector = detector_with(2.0, 8, 20);
  detector.record_heartbeat("node-a:4300", 0);
  detector.record_heartbeat("node-a:4300", 1);

  assert!(detector.poll(40).is_empty(), "minimum interval should prevent early suspect");
  let effects = detector.poll(41);
  assert!(
    matches!(effects.as_slice(), [PhiFailureDetectorEffect::Suspect { authority, .. }] if authority == "node-a:4300")
  );
}

#[test]
fn authorities_are_tracked_independently() {
  let mut detector = detector_with(1.5, 8, 1);
  detector.record_heartbeat("node-a:4400", 0);
  detector.record_heartbeat("node-a:4400", 10);

  detector.record_heartbeat("node-b:4401", 0);
  detector.record_heartbeat("node-b:4401", 10);
  detector.record_heartbeat("node-b:4401", 38);

  let effects = detector.poll(40);
  assert_eq!(effects.len(), 1);
  assert!(
    matches!(effects.as_slice(), [PhiFailureDetectorEffect::Suspect { authority, .. }] if authority == "node-a:4400")
  );
}
