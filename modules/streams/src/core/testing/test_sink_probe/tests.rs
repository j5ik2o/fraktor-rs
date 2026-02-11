use crate::core::{StreamError, TestSinkProbe};

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
