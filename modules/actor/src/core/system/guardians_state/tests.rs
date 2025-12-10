use super::GuardianSlot;
use crate::core::{
  actor_prim::Pid,
  system::{GuardianKind, guardians_state::GuardiansState},
};

#[test]
fn guardians_state_register_and_pid() {
  let mut state = GuardiansState::new();
  let root = Pid::new(1, 0);
  let user = Pid::new(2, 0);
  state.register(GuardianKind::Root, root);
  state.register(GuardianKind::User, user);

  assert_eq!(state.pid(GuardianKind::Root), Some(root));
  assert_eq!(state.pid(GuardianKind::User), Some(user));
}

#[test]
fn guardians_state_clear_by_pid_marks_unset() {
  let mut state = GuardiansState::new();
  let system_pid = Pid::new(3, 0);
  state.register(GuardianKind::System, system_pid);

  let cleared = state.clear_by_pid(system_pid);
  assert_eq!(cleared, Some(GuardianKind::System));
  assert!(!state.is_alive(GuardianKind::System));
  assert_eq!(state.pid(GuardianKind::System), Some(system_pid));
}

#[test]
fn guardians_state_tracks_alive_flag() {
  let mut state = GuardiansState::new();
  let root = Pid::new(11, 0);
  state.register(GuardianKind::Root, root);
  assert!(state.is_alive(GuardianKind::Root));

  let cleared = state.clear_by_pid(root);
  assert_eq!(cleared, Some(GuardianKind::Root));
  assert!(!state.is_alive(GuardianKind::Root));
  assert_eq!(state.pid(GuardianKind::Root), Some(root));
}

#[test]
fn guardians_state_overwrites_pid_and_revives() {
  let mut state = GuardiansState::new();
  let old_pid = Pid::new(21, 0);
  let new_pid = Pid::new(22, 0);

  state.register(GuardianKind::User, old_pid);
  assert_eq!(state.pid(GuardianKind::User), Some(old_pid));
  assert!(state.is_alive(GuardianKind::User));

  let cleared = state.clear_by_pid(old_pid);
  assert_eq!(cleared, Some(GuardianKind::User));
  assert!(!state.is_alive(GuardianKind::User));

  state.register(GuardianKind::User, new_pid);
  assert_eq!(state.pid(GuardianKind::User), Some(new_pid));
  assert!(state.is_alive(GuardianKind::User));
}

#[test]
fn guardian_slot_set_restores_alive_flag() {
  let mut slot = GuardianSlot::new(Pid::new(31, 0));
  let new_pid = Pid::new(32, 0);

  slot.clear();
  assert!(!slot.is_alive());

  slot.set(new_pid);
  assert_eq!(slot.pid(), new_pid);
  assert!(slot.is_alive());
}
