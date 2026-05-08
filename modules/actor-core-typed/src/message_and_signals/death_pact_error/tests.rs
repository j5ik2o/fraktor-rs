use alloc::{format, string::ToString};

use fraktor_actor_core_rs::core::kernel::actor::{
  Pid,
  error::{ActorError, ActorErrorReason},
};

use crate::message_and_signals::DeathPactError;

#[test]
fn new_stores_terminated_pid() {
  let pid = Pid::new(42, 0);
  let ex = DeathPactError::new(pid);
  assert_eq!(ex.terminated(), pid);
}

#[test]
fn display_contains_pid() {
  let pid = Pid::new(7, 3);
  let ex = DeathPactError::new(pid);
  let msg = format!("{}", ex);
  assert!(msg.contains("death pact"), "message should describe death pact: {msg}");
  assert!(msg.contains(&pid.to_string()), "message should contain the pid: {msg}");
}

#[test]
fn equality() {
  let a = DeathPactError::new(Pid::new(1, 0));
  let b = DeathPactError::new(Pid::new(1, 0));
  let c = DeathPactError::new(Pid::new(2, 0));
  assert_eq!(a, b);
  assert_ne!(a, c);
}

#[test]
fn typed_error_reason_identifies_death_pact() {
  let pid = Pid::new(10, 0);
  let ex = DeathPactError::new(pid);
  let reason = ActorErrorReason::typed::<DeathPactError>(ex.to_string());
  assert!(reason.is_source_type::<DeathPactError>());
}

#[test]
fn actor_error_carries_death_pact_type() {
  let pid = Pid::new(20, 0);
  let ex = DeathPactError::new(pid);
  let error = ActorError::recoverable_typed::<DeathPactError>(ex.to_string());
  assert!(error.is_source_type::<DeathPactError>());
  assert!(error.reason().as_str().contains("death pact"));
}

#[test]
fn clone_preserves_fields() {
  let ex = DeathPactError::new(Pid::new(5, 1));
  let cloned = ex.clone();
  assert_eq!(ex, cloned);
}
