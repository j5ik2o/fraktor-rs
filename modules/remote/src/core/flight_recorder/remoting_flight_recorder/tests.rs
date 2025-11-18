use fraktor_actor_rs::core::{event_stream::CorrelationId, system::AuthorityState};

use crate::core::{
  flight_recorder::{CorrelationTrace, CorrelationTraceHop, RemotingFlightRecorder, RemotingMetric},
  remoting_connection_snapshot::RemotingConnectionSnapshot,
};

#[test]
fn ring_buffer_retains_latest_metrics() {
  let recorder = RemotingFlightRecorder::new(2);
  recorder.record_metric(RemotingMetric::new("node-a").with_latency_ms(10));
  recorder.record_metric(RemotingMetric::new("node-b").with_latency_ms(20));
  recorder.record_metric(RemotingMetric::new("node-c").with_latency_ms(30));

  let snapshot = recorder.metrics_snapshot();
  assert_eq!(snapshot.len(), 2);
  assert_eq!(snapshot[0].authority(), "node-b");
  assert_eq!(snapshot[1].authority(), "node-c");
}

#[test]
fn correlation_traces_are_recorded() {
  let recorder = RemotingFlightRecorder::new(4);
  let trace = CorrelationTrace::new(CorrelationId::new(1, 2), "node-a", CorrelationTraceHop::Send);
  recorder.record_trace(trace.clone());

  let traces = recorder.traces_snapshot();
  assert_eq!(traces, vec![trace]);
}

#[test]
fn endpoint_snapshot_is_reported() {
  let recorder = RemotingFlightRecorder::new(4);
  let snapshot = RemotingConnectionSnapshot::new("node-a", AuthorityState::Connected);
  recorder.update_endpoint_snapshot(vec![snapshot.clone()]);

  let snapshots = recorder.endpoint_snapshot();
  assert_eq!(snapshots, vec![snapshot]);
}
