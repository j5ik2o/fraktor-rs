use crate::core::recovery_timed_out::RecoveryTimedOut;

#[test]
fn recovery_timed_out_keeps_persistence_id() {
  let signal = RecoveryTimedOut::new("pid-1");

  assert_eq!(signal.persistence_id(), "pid-1");
}
