#![cfg(not(target_os = "none"))]

use std::panic::{AssertUnwindSafe, catch_unwind};

use fraktor_actor_core_rs::core::kernel::actor::{
  error::ActorError,
  invoke_guard::{InvokeGuard, InvokeGuardFactory, NoopInvokeGuard, NoopInvokeGuardFactory},
};
use fraktor_utils_core_rs::core::sync::ArcShared;

#[test]
fn noop_invoke_guard_wrap_passes_through_ok_and_err() {
  let guard = NoopInvokeGuard::new();

  assert_eq!(guard.wrap(|| Ok(())), Ok(()));
  assert!(matches!(guard.wrap(|| Err(ActorError::recoverable("planned"))), Err(ActorError::Recoverable(_))));
}

#[test]
fn noop_invoke_guard_does_not_catch_panic() {
  let guard = NoopInvokeGuard::new();

  let result = catch_unwind(AssertUnwindSafe(|| {
    let _ = guard.wrap(|| panic!("boom"));
  }));

  assert!(result.is_err());
}

#[test]
fn noop_invoke_guard_factory_builds_dyn_compatible_guard() {
  let factory = NoopInvokeGuardFactory::new();
  let guard: ArcShared<Box<dyn InvokeGuard>> = factory.build();

  let result = guard.wrap_receive(&mut || Ok(()));
  assert_eq!(result, Ok(()));
}
