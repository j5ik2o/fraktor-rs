#![cfg(not(target_os = "none"))]

use std::panic::{AssertUnwindSafe, catch_unwind};

use fraktor_actor_adaptor_std_rs::std::actor::PanicInvokeGuard;
use fraktor_actor_core_rs::core::kernel::actor::{
  error::ActorError,
  invoke_guard::{InvokeGuard, NoopInvokeGuard},
};

#[test]
fn sp_h1_5_t1_noop_guard_propagates_panic() {
  let guard = NoopInvokeGuard::new();

  let result = catch_unwind(AssertUnwindSafe(|| {
    let _ = guard.wrap(|| panic!("boom"));
  }));

  assert!(result.is_err());
}

#[test]
fn sp_h1_5_t2_panic_guard_converts_panic_to_escalate() {
  let guard = PanicInvokeGuard::new();

  let result = guard.wrap(|| panic!("panic guard boom"));

  assert!(matches!(result, Err(ActorError::Escalate(reason)) if reason.as_str().contains("panic guard boom")));
}

#[test]
fn sp_h1_5_t3_panic_guard_preserves_actor_errors() {
  let guard = PanicInvokeGuard::new();

  assert!(matches!(guard.wrap(|| Err(ActorError::recoverable("planned"))), Err(ActorError::Recoverable(_))));
  assert!(matches!(guard.wrap(|| Err(ActorError::fatal("fatal"))), Err(ActorError::Fatal(_))));
}

#[test]
fn sp_h1_5_t4_panic_guard_preserves_ok_result() {
  let guard = PanicInvokeGuard::new();

  assert_eq!(guard.wrap(|| Ok(())), Ok(()));
}
