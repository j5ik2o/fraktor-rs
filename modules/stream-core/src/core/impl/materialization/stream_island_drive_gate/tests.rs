use crate::core::r#impl::materialization::StreamIslandDriveGate;

fn new_drive_gate() -> StreamIslandDriveGate {
  StreamIslandDriveGate::new()
}

#[test]
fn drive_gate_rejects_second_pending_drive_until_idle() {
  let gate = new_drive_gate();

  assert!(gate.try_mark_pending());
  assert!(!gate.try_mark_pending());

  gate.mark_idle();

  assert!(gate.try_mark_pending());
}

#[test]
fn drive_gate_clone_shares_pending_state() {
  let gate = new_drive_gate();
  let cloned = gate.clone();

  assert!(gate.try_mark_pending());

  assert!(!cloned.try_mark_pending());
}
