use crate::core::{SharedKillSwitch, StreamError};

#[test]
fn shared_kill_switch_shutdown_is_visible_across_clones() {
  let switch = SharedKillSwitch::new();
  let cloned = switch.clone();
  switch.shutdown();
  assert!(cloned.is_shutdown());
  assert!(!cloned.is_aborted());
}

#[test]
fn shared_kill_switch_abort_is_visible_across_clones() {
  let switch = SharedKillSwitch::new();
  let cloned = switch.clone();
  cloned.abort(StreamError::Failed);
  assert!(switch.is_aborted());
  assert_eq!(switch.abort_error(), Some(StreamError::Failed));
}

#[test]
fn shared_kill_switch_keeps_first_control_signal_across_clones() {
  let switch = SharedKillSwitch::new();
  let cloned = switch.clone();

  cloned.shutdown();
  switch.abort(StreamError::Failed);

  assert!(switch.is_shutdown());
  assert!(cloned.is_shutdown());
  assert!(!switch.is_aborted());
  assert_eq!(switch.abort_error(), None);
}
