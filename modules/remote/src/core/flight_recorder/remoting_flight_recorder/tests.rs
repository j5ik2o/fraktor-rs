use fraktor_actor_rs::core::event_stream::{BackpressureSignal, CorrelationId};

use super::{FlightMetricKind, RemotingFlightRecorder};

fn correlation(id: u128) -> CorrelationId {
  CorrelationId::from_u128(id)
}

#[test]
fn stores_backpressure_and_suspect_records() {
  let recorder = RemotingFlightRecorder::new(4);
  recorder.record_backpressure("loopback:4100", BackpressureSignal::Apply, correlation(1), 10);
  recorder.record_suspect("loopback:4100", 2.5, correlation(2), 20);
  recorder.record_reachable("loopback:4100", correlation(3), 30);

  let snapshot = recorder.snapshot();
  assert_eq!(snapshot.records().len(), 3);
  assert!(matches!(snapshot.records()[0].kind(), FlightMetricKind::Backpressure(BackpressureSignal::Apply)));
  assert!(matches!(snapshot.records()[1].kind(), FlightMetricKind::Suspect { .. }));
  assert!(matches!(snapshot.records()[2].kind(), FlightMetricKind::Reachable));
}

#[test]
fn ring_buffer_discards_oldest_entries() {
  let recorder = RemotingFlightRecorder::new(2);
  recorder.record_backpressure("a", BackpressureSignal::Apply, correlation(1), 1);
  recorder.record_backpressure("b", BackpressureSignal::Apply, correlation(2), 2);
  recorder.record_backpressure("c", BackpressureSignal::Apply, correlation(3), 3);

  let snapshot = recorder.snapshot();
  assert_eq!(snapshot.records().len(), 2);
  assert_eq!(snapshot.records()[0].authority(), "b");
  assert_eq!(snapshot.records()[1].authority(), "c");
}

#[test]
fn preserves_correlation_ids_across_metrics() {
  let recorder = RemotingFlightRecorder::new(4);
  let correlation_id = correlation(99);
  recorder.record_backpressure("loopback:4500", BackpressureSignal::Apply, correlation_id, 10);
  recorder.record_suspect("loopback:4500", 3.2, correlation_id, 20);

  let snapshot = recorder.snapshot();
  assert_eq!(snapshot.records().len(), 2);
  assert!(
    snapshot
      .records()
      .iter()
      .all(|metric| metric.correlation_id() == correlation_id && metric.authority() == "loopback:4500")
  );
}
