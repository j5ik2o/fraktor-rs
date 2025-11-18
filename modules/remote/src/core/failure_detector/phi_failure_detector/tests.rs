use super::{PhiFailureDetector, PhiFailureDetectorConfig, PhiFailureDetectorEffect};

fn detector() -> PhiFailureDetector {
  PhiFailureDetector::new(PhiFailureDetectorConfig::new(1.5, 4, 1))
}

#[test]
fn emits_suspect_after_threshold() {
  let mut detector = detector();
  detector.record_heartbeat("loopback:4100", 0);
  detector.record_heartbeat("loopback:4100", 10);
  let effects = detector.poll(40);
  assert!(
    matches!(effects.as_slice(), [PhiFailureDetectorEffect::Suspect { authority, .. }] if authority == "loopback:4100")
  );
}

#[test]
fn emits_reachable_after_new_heartbeat() {
  let mut detector = detector();
  detector.record_heartbeat("loopback:4200", 0);
  detector.record_heartbeat("loopback:4200", 10);
  let suspect = detector.poll(40);
  assert!(matches!(suspect.as_slice(), [PhiFailureDetectorEffect::Suspect { .. }]));

  let reachable = detector.record_heartbeat("loopback:4200", 42);
  assert!(matches!(reachable, Some(PhiFailureDetectorEffect::Reachable { authority }) if authority == "loopback:4200"));
}
