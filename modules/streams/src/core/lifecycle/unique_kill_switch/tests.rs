use crate::core::{StreamError, UniqueKillSwitch};

#[test]
fn unique_kill_switch_shutdown_sets_state() {
  let switch = UniqueKillSwitch::new();
  switch.shutdown();
  assert!(switch.is_shutdown());
  assert!(!switch.is_aborted());
}

#[test]
fn unique_kill_switch_abort_sets_error() {
  let switch = UniqueKillSwitch::new();
  switch.abort(StreamError::Failed);
  assert!(switch.is_aborted());
  assert_eq!(switch.abort_error(), Some(StreamError::Failed));
}

#[test]
fn unique_kill_switch_keeps_first_control_signal() {
  let switch = UniqueKillSwitch::new();

  switch.shutdown();
  switch.abort(StreamError::Failed);

  assert!(switch.is_shutdown());
  assert!(!switch.is_aborted());
  assert_eq!(switch.abort_error(), None);
}
