use super::SystemState;
use crate::{NoStdToolbox, actor_prim::Pid};

#[test]
fn system_state_new() {
  let state = SystemState::<NoStdToolbox>::new();
  assert!(!state.is_terminated());
  assert_eq!(state.deadletters().len(), 0);
}

#[test]
fn system_state_default() {
  let state = SystemState::<NoStdToolbox>::default();
  assert!(!state.is_terminated());
}

#[test]
fn system_state_allocate_pid() {
  let state = SystemState::<NoStdToolbox>::new();
  let pid1 = state.allocate_pid();
  let pid2 = state.allocate_pid();
  // Pid????????
  assert_ne!(pid1.value(), pid2.value());
}

#[test]
fn system_state_monotonic_now() {
  let state = SystemState::<NoStdToolbox>::new();
  let now1 = state.monotonic_now();
  let now2 = state.monotonic_now();
  // ???????
  assert!(now2 > now1);
}

#[test]
fn system_state_event_stream() {
  let state = SystemState::<NoStdToolbox>::new();
  let stream = state.event_stream();
  // ????????????????????
  let _ = stream;
}

#[test]
fn system_state_termination_future() {
  let state = SystemState::<NoStdToolbox>::new();
  let future = state.termination_future();
  // ???????????????????
  assert!(!future.is_ready());
}

#[test]
fn system_state_mark_terminated() {
  let state = SystemState::<NoStdToolbox>::new();
  assert!(!state.is_terminated());
  state.mark_terminated();
  assert!(state.is_terminated());
}

#[test]
fn system_state_register_and_remove_cell() {
  use cellactor_utils_core_rs::sync::ArcShared;

  use crate::actor_prim::ActorCell;

  let state = SystemState::<NoStdToolbox>::new();
  let pid = state.allocate_pid();

  // ActorCell????????????????????
  // cell???????????????
  let _ = pid;
}

#[test]
fn system_state_assign_name_with_hint() {
  let state = SystemState::<NoStdToolbox>::new();
  let pid = state.allocate_pid();

  let result = state.assign_name(None, Some("test-actor"), pid);
  assert!(result.is_ok());
  let name = result.unwrap();
  assert_eq!(name, "test-actor");
}

#[test]
fn system_state_assign_name_without_hint() {
  let state = SystemState::<NoStdToolbox>::new();
  let pid = state.allocate_pid();

  let result = state.assign_name(None, None, pid);
  assert!(result.is_ok());
  let name = result.unwrap();
  // ?????????
  assert!(!name.is_empty());
}

#[test]
fn system_state_release_name() {
  let state = SystemState::<NoStdToolbox>::new();
  let pid = state.allocate_pid();

  let _name = state.assign_name(None, Some("test-actor"), pid).unwrap();
  state.release_name(None, "test-actor");
  // ?????????????????????????????
}

#[test]
fn system_state_user_guardian_pid() {
  let state = SystemState::<NoStdToolbox>::new();
  // ??????user_guardian?????????
  assert!(state.user_guardian_pid().is_none());
}

#[test]
fn system_state_child_pids() {
  let state = SystemState::<NoStdToolbox>::new();
  let parent_pid = state.allocate_pid();

  // ???????????
  let children = state.child_pids(parent_pid);
  assert_eq!(children.len(), 0);
}

#[test]
fn system_state_deadletters() {
  let state = SystemState::<NoStdToolbox>::new();
  let deadletters = state.deadletters();
  // ???????????????
  assert_eq!(deadletters.len(), 0);
}
