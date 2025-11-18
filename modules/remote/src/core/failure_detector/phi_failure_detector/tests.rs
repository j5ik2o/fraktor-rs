use core::time::Duration;

use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::{
  endpoint_manager::{AssociationState, EndpointManager, QuarantineReason},
  failure_detector::{
    failure_detector_event::FailureDetectorEvent, phi_failure_detector::PhiFailureDetector,
    phi_failure_detector_config::PhiFailureDetectorConfig,
  },
  flight_recorder::remoting_flight_recorder::RemotingFlightRecorder,
};

#[test]
fn suspect_and_reachable_events_are_emitted() {
  let recorder = RemotingFlightRecorder::new(8);
  let detector: PhiFailureDetector<NoStdToolbox> =
    PhiFailureDetector::new(PhiFailureDetectorConfig::default().with_threshold(1.0), recorder.clone());

  for tick in [100_u64, 200, 300, 400, 500] {
    detector.record_heartbeat("node-a", Duration::from_millis(tick));
  }

  let events = detector.detect(Duration::from_millis(2000));
  assert!(matches!(events.first(), Some(FailureDetectorEvent::Suspect { authority, .. }) if authority == "node-a"));
  assert_eq!(recorder.suspect_events("node-a"), 1);

  detector.record_heartbeat("node-a", Duration::from_millis(2010));
  let events = detector.detect(Duration::from_millis(2020));
  assert!(matches!(events.first(), Some(FailureDetectorEvent::Reachable { authority }) if authority == "node-a"));
}

#[test]
fn custom_threshold_delays_suspect_detection() {
  let recorder = RemotingFlightRecorder::new(8);
  let detector: PhiFailureDetector<NoStdToolbox> =
    PhiFailureDetector::new(PhiFailureDetectorConfig::default().with_threshold(50.0), recorder);

  for tick in [100_u64, 200, 300, 400, 500] {
    detector.record_heartbeat("node-b", Duration::from_millis(tick));
  }

  let events = detector.detect(Duration::from_millis(2000));
  assert!(events.is_empty(), "threshold should delay suspect event");
}

#[test]
fn suspect_events_drive_quarantine_and_recorder() {
  let recorder = RemotingFlightRecorder::new(8);
  let detector: PhiFailureDetector<NoStdToolbox> =
    PhiFailureDetector::new(PhiFailureDetectorConfig::default().with_threshold(1.0), recorder.clone());
  let manager = EndpointManager::new();

  for tick in [100_u64, 200, 300, 400, 500] {
    detector.record_heartbeat("node-c", Duration::from_millis(tick));
  }

  let events = detector.detect(Duration::from_millis(2000));
  assert!(matches!(
    events.first(),
    Some(FailureDetectorEvent::Suspect { authority, .. }) if authority == "node-c"
  ));

  for event in events {
    if let FailureDetectorEvent::Suspect { authority, .. } = event {
      manager.set_quarantine(&authority, QuarantineReason::Manual("suspect".to_string()), 2_000, None);
    }
  }

  let snapshot = manager.snapshots();
  assert!(snapshot.iter().any(|entry| matches!(entry.state(), AssociationState::Quarantined { .. })));
  assert_eq!(recorder.suspect_events("node-c"), 1);

  detector.record_heartbeat("node-c", Duration::from_millis(2010));
  let reachable = detector.detect(Duration::from_millis(2020));
  assert!(
    reachable
      .iter()
      .any(|event| matches!(event, FailureDetectorEvent::Reachable { authority } if authority == "node-c"))
  );
  assert_eq!(recorder.reachable_events("node-c"), 1);
}
