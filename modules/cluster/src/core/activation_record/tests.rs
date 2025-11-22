use crate::core::activation_record::ActivationRecord;

#[test]
fn keeps_snapshot_and_version() {
  let record = ActivationRecord::new("pid-1".to_string(), Some(vec![1, 2, 3]), 7);
  assert_eq!(record.pid, "pid-1");
  assert_eq!(record.snapshot, Some(vec![1, 2, 3]));
  assert_eq!(record.version, 7);
}
