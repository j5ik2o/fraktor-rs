use crate::core::TestSourceProbe;

#[test]
fn test_source_probe_push_pull_and_complete() {
  let mut probe = TestSourceProbe::new();
  probe.push(1_u32);
  probe.push(2_u32);
  assert_eq!(probe.pull(), Some(1_u32));
  assert_eq!(probe.pull(), Some(2_u32));
  assert_eq!(probe.pull(), None);
  probe.complete();
  assert!(probe.is_completed());
}
