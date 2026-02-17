use crate::core::lifecycle::KillSwitches;

#[test]
fn kill_switches_shared_returns_shared_kill_switch() {
  let switch = KillSwitches::shared();
  assert!(!switch.is_shutdown());
  assert!(!switch.is_aborted());
}

#[test]
fn kill_switches_single_returns_unique_kill_switch() {
  let switch = KillSwitches::single();
  assert!(!switch.is_shutdown());
  assert!(!switch.is_aborted());
}
