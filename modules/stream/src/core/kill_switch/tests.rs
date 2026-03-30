use crate::core::{KillSwitch, SharedKillSwitch, StreamError, UniqueKillSwitch};

fn assert_kill_switch_contract<T>(switch: &T)
where
  T: KillSwitch, {
  assert!(!switch.is_shutdown());
  assert!(!switch.is_aborted());
  assert_eq!(switch.abort_error(), None);
}

#[test]
fn shared_kill_switch_implements_contract() {
  let switch = SharedKillSwitch::new();
  assert_kill_switch_contract(&switch);

  switch.abort(StreamError::Failed);
  assert!(switch.is_aborted());
  assert_eq!(switch.abort_error(), Some(StreamError::Failed));
}

#[test]
fn unique_kill_switch_implements_contract() {
  let switch = UniqueKillSwitch::new();
  assert_kill_switch_contract(&switch);

  switch.shutdown();
  assert!(switch.is_shutdown());
  assert!(!switch.is_aborted());
}
