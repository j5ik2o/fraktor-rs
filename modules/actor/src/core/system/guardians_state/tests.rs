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
  assert_eq!(state.kind_by_pid(root), Some(GuardianKind::Root));
  assert_eq!(state.kind_by_pid(user), Some(GuardianKind::User));
}

#[test]
fn guardians_state_kind_by_pid_returns_none_for_unknown_pid() {
  let mut state = GuardiansState::new();
  state.register(GuardianKind::System, Pid::new(3, 0));

  assert_eq!(state.kind_by_pid(Pid::new(4, 0)), None);
}

#[test]
fn guardians_state_overwrites_pid() {
  let mut state = GuardiansState::new();
  let old_pid = Pid::new(21, 0);
  let new_pid = Pid::new(22, 0);

  state.register(GuardianKind::User, old_pid);
  assert_eq!(state.pid(GuardianKind::User), Some(old_pid));

  state.register(GuardianKind::User, new_pid);
  assert_eq!(state.pid(GuardianKind::User), Some(new_pid));
  assert_eq!(state.kind_by_pid(old_pid), None);
  assert_eq!(state.kind_by_pid(new_pid), Some(GuardianKind::User));
}
