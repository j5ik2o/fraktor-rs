use alloc::string::String;

use crate::core::{
  address::Address,
  extension::{RemoteAuthoritySnapshot, RemotingError, RemotingLifecycleState},
};

// ---------------------------------------------------------------------------
// RemotingLifecycleState — happy paths
// ---------------------------------------------------------------------------

#[test]
fn new_state_is_pending() {
  let s = RemotingLifecycleState::new();
  assert!(!s.is_running());
  assert!(!s.is_terminated());
  assert_eq!(s.ensure_running().unwrap_err(), RemotingError::NotStarted);
}

#[test]
fn pending_to_starting_to_running_to_shuttingdown_to_shutdown() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap(); // Pending → Starting
  assert!(!s.is_running(), "Starting is not Running yet");
  s.mark_started().unwrap(); // Starting → Running
  assert!(s.is_running());
  s.ensure_running().unwrap();
  s.transition_to_shutdown().unwrap(); // Running → ShuttingDown
  assert!(!s.is_running());
  s.mark_shutdown().unwrap(); // ShuttingDown → Shutdown
  assert!(s.is_terminated());
  assert_eq!(s.ensure_running().unwrap_err(), RemotingError::NotStarted);
}

#[test]
fn pending_can_shortcut_to_shutdown() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_shutdown().unwrap(); // Pending → Shutdown
  assert!(s.is_terminated());
}

// ---------------------------------------------------------------------------
// RemotingLifecycleState — invalid transitions
// ---------------------------------------------------------------------------

#[test]
fn mark_started_from_pending_is_invalid_transition() {
  let mut s = RemotingLifecycleState::new();
  assert_eq!(s.mark_started().unwrap_err(), RemotingError::InvalidTransition);
}

#[test]
fn transition_to_start_from_starting_is_already_running() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  assert_eq!(s.transition_to_start().unwrap_err(), RemotingError::AlreadyRunning);
}

#[test]
fn transition_to_start_from_running_is_already_running() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  s.mark_started().unwrap();
  assert_eq!(s.transition_to_start().unwrap_err(), RemotingError::AlreadyRunning);
}

#[test]
fn transition_to_start_from_shutdown_is_invalid_transition() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_shutdown().unwrap(); // Pending → Shutdown
  assert_eq!(s.transition_to_start().unwrap_err(), RemotingError::InvalidTransition);
}

#[test]
fn transition_to_shutdown_from_starting_is_invalid_transition() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  assert_eq!(s.transition_to_shutdown().unwrap_err(), RemotingError::InvalidTransition);
}

#[test]
fn transition_to_shutdown_from_shutdown_is_invalid_transition() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_shutdown().unwrap();
  assert_eq!(s.transition_to_shutdown().unwrap_err(), RemotingError::InvalidTransition);
}

#[test]
fn mark_shutdown_from_running_is_invalid_transition() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  s.mark_started().unwrap();
  assert_eq!(s.mark_shutdown().unwrap_err(), RemotingError::InvalidTransition);
}

#[test]
fn ensure_running_from_starting_returns_not_started() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  assert_eq!(s.ensure_running().unwrap_err(), RemotingError::NotStarted);
}

#[test]
fn ensure_running_from_shutting_down_returns_not_started() {
  let mut s = RemotingLifecycleState::new();
  s.transition_to_start().unwrap();
  s.mark_started().unwrap();
  s.transition_to_shutdown().unwrap();
  assert_eq!(s.ensure_running().unwrap_err(), RemotingError::NotStarted);
}

// ---------------------------------------------------------------------------
// RemoteAuthoritySnapshot
// ---------------------------------------------------------------------------

#[test]
fn remote_authority_snapshot_exposes_all_fields() {
  let addr = Address::new("sys", "host", 2552);
  let snap = RemoteAuthoritySnapshot::new(addr.clone(), true, false, Some(10_000), Some(String::from("fine")));
  assert_eq!(snap.address(), &addr);
  assert!(snap.is_connected());
  assert!(!snap.is_quarantined());
  assert_eq!(snap.last_contact_ms(), Some(10_000));
  assert_eq!(snap.quarantine_reason(), Some("fine"));
}

#[test]
fn remote_authority_snapshot_clone_preserves_fields() {
  let snap = RemoteAuthoritySnapshot::new(Address::new("sys", "host", 0), false, true, None, None);
  let cloned = snap.clone();
  assert_eq!(snap, cloned);
}
