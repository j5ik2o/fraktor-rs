use super::StreamHandleState;
use crate::core::{drive_outcome::DriveOutcome, stream_error::StreamError, stream_state::StreamState};

#[test]
fn running_handle_transitions() {
  let mut handle = StreamHandleState::running();
  assert_eq!(handle.state(), StreamState::Running);
  assert!(handle.complete().is_ok());
  assert_eq!(handle.state(), StreamState::Completed);
}

#[test]
fn running_handle_can_fail() {
  let mut handle = StreamHandleState::running();
  assert!(handle.fail().is_ok());
  assert_eq!(handle.state(), StreamState::Failed);
}

#[test]
fn cancel_requires_running_state() {
  let mut handle = StreamHandleState::new();
  assert_eq!(handle.cancel(), Err(StreamError::NotRunning));
  assert_eq!(handle.state(), StreamState::Idle);
}

#[test]
fn drive_consumes_demand() {
  let mut handle = StreamHandleState::running();
  assert_eq!(handle.drive(), DriveOutcome::Idle);
  assert!(handle.request(1).is_ok());
  assert_eq!(handle.drive(), DriveOutcome::Progressed);
  assert_eq!(handle.drive(), DriveOutcome::Idle);
}
