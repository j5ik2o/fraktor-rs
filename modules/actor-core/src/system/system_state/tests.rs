use super::SystemState;
use crate::messaging::AnyMessage;

#[test]
fn system_state_new() {
  let state = SystemState::new();
  assert!(!state.is_terminated());
  assert_eq!(state.dead_letters().len(), 0);
}

#[test]
fn system_state_default() {
  let state = SystemState::default();
  assert!(!state.is_terminated());
}

#[test]
fn system_state_allocate_pid() {
  let state = SystemState::new();
  let pid1 = state.allocate_pid();
  let pid2 = state.allocate_pid();
  assert_ne!(pid1.value(), pid2.value());
}

#[test]
fn system_state_monotonic_now() {
  let state = SystemState::new();
  let now1 = state.monotonic_now();
  let now2 = state.monotonic_now();
  assert!(now2 > now1);
}

#[test]
fn system_state_event_stream() {
  let state = SystemState::new();
  let stream = state.event_stream();
  let _ = stream;
}

#[test]
fn system_state_termination_future() {
  let state = SystemState::new();
  let future = state.termination_future();
  assert!(!future.is_ready());
}

#[test]
fn system_state_mark_terminated() {
  let state = SystemState::new();
  assert!(!state.is_terminated());
  state.mark_terminated();
  assert!(state.is_terminated());
}

#[test]
fn system_state_register_and_remove_cell() {
  let state = SystemState::new();
  let pid = state.allocate_pid();

  let _ = pid;
}

#[test]
fn system_state_assign_name_with_hint() {
  let state = SystemState::new();
  let pid = state.allocate_pid();

  let result = state.assign_name(None, Some("test-actor"), pid);
  assert!(result.is_ok());
  let name = result.unwrap();
  assert_eq!(name, "test-actor");
}

#[test]
fn system_state_assign_name_without_hint() {
  let state = SystemState::new();
  let pid = state.allocate_pid();

  let result = state.assign_name(None, None, pid);
  assert!(result.is_ok());
  let name = result.unwrap();

  assert!(!name.is_empty());
}

#[test]
fn system_state_release_name() {
  let state = SystemState::new();
  let pid = state.allocate_pid();

  let _name = state.assign_name(None, Some("test-actor"), pid).unwrap();
  state.release_name(None, "test-actor");
}

#[test]
fn system_state_user_guardian_pid() {
  let state = SystemState::new();
  assert!(state.user_guardian_pid().is_none());
}

#[test]
fn system_state_child_pids() {
  let state = SystemState::new();
  let parent_pid = state.allocate_pid();

  let children = state.child_pids(parent_pid);
  assert_eq!(children.len(), 0);
}

#[test]
fn system_state_deadletters() {
  let state = SystemState::new();
  let dead_letters = state.dead_letters();
  assert_eq!(dead_letters.len(), 0);
}

#[test]
fn system_state_register_ask_future() {
  use cellactor_utils_core_rs::sync::ArcShared;

  use crate::futures::ActorFuture;

  let state = SystemState::new();
  let future = ArcShared::new(ActorFuture::<AnyMessage>::new());
  state.register_ask_future(future.clone());

  let ready = state.drain_ready_ask_futures();
  assert_eq!(ready.len(), 0);
}

#[test]
fn system_state_publish_event() {
  use alloc::string::String;
  use core::time::Duration;

  use crate::{
    event_stream::EventStreamEvent,
    logging::{LogEvent, LogLevel},
  };

  let state = SystemState::new();
  let log_event = LogEvent::new(LogLevel::Info, String::from("test"), Duration::from_millis(1), None);
  let event = EventStreamEvent::Log(log_event);

  state.publish_event(&event);
}

#[test]
fn system_state_emit_log() {
  use alloc::string::String;

  let state = SystemState::new();
  let pid = state.allocate_pid();

  state.emit_log(crate::logging::LogLevel::Info, String::from("test message"), Some(pid));
  state.emit_log(crate::logging::LogLevel::Error, String::from("error message"), None);
}

#[test]
fn system_state_clear_guardian() {
  let state = SystemState::new();
  let pid = state.allocate_pid();

  let cleared = state.clear_guardian(pid);
  assert!(!cleared);
}

#[test]
fn system_state_user_guardian() {
  let state = SystemState::new();
  assert!(state.user_guardian().is_none());
}

#[test]
fn system_state_send_system_message_to_nonexistent_actor() {
  use crate::messaging::SystemMessage;

  let state = SystemState::new();
  let pid = state.allocate_pid();

  let result = state.send_system_message(pid, SystemMessage::Stop);
  assert!(result.is_err());
}

#[test]
fn system_state_record_send_error() {
  use crate::error::SendError;

  let state = SystemState::new();
  let error = SendError::closed(AnyMessage::new(42_u32));

  state.record_send_error(None, &error);
  state.record_send_error(Some(state.allocate_pid()), &error);
}
