use crate::core::{
  StreamError,
  testing::{TestSinkProbe, TestSourceProbe},
};

#[test]
fn test_sink_probe_respects_demand() {
  let mut probe = TestSinkProbe::new();
  assert_eq!(probe.push(1_u32), Err(StreamError::DemandExceeded { requested: 1, remaining: 0 }));
  probe.request(2);
  probe.push(1_u32).expect("push");
  probe.push(2_u32).expect("push");
  assert_eq!(probe.received(), &[1_u32, 2_u32]);
}

#[test]
fn test_sink_probe_tracks_completion_and_failure() {
  let mut probe = TestSinkProbe::<u32>::new();
  probe.complete();
  probe.fail(StreamError::Failed);
  assert!(probe.is_completed());
  assert_eq!(probe.failed(), Some(StreamError::Failed));
}

#[test]
fn source_sink_probe_scenario_covers_demand_failure_and_completion() {
  let mut source = TestSourceProbe::new();
  let mut sink = TestSinkProbe::new();

  source.push(10_u32);
  source.push(20_u32);

  let first = source.pull().expect("first pull");
  assert_eq!(sink.push(first), Err(StreamError::DemandExceeded { requested: 1, remaining: 0 }));

  sink.request(1);
  let second = source.pull().expect("second pull");
  sink.push(second).expect("push with demand");

  source.complete();
  sink.fail(StreamError::Failed);
  sink.complete();

  assert!(source.is_completed());
  assert!(sink.is_completed());
  assert_eq!(sink.failed(), Some(StreamError::Failed));
  assert_eq!(sink.received(), &[20_u32]);
}
