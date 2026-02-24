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

#[test]
fn kill_switches_single_bidi_returns_bidi_flow_with_kill_switch() {
  let bidi = KillSwitches::single_bidi::<u32, u32>();
  let (_, _, switch) = bidi.split();
  assert!(!switch.is_shutdown());
  assert!(!switch.is_aborted());
}

#[test]
fn kill_switches_single_bidi_shutdown_propagates_to_switch() {
  let bidi = KillSwitches::single_bidi::<u32, u32>();
  let (_top, _bottom, switch) = bidi.split();
  switch.shutdown();
  assert!(switch.is_shutdown());
}
