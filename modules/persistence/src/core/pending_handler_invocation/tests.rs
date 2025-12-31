use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{pending_handler_invocation::PendingHandlerInvocation, persistent_repr::PersistentRepr};

struct Counter {
  calls: usize,
}

#[test]
fn pending_handler_invocation_invoke_stashing() {
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_i32);
  let repr = PersistentRepr::new("pid-1", 1, payload);
  let mut counter = Counter { calls: 0 };

  let invocation = PendingHandlerInvocation::stashing(repr.clone(), |actor: &mut Counter, _| {
    actor.calls += 1;
  });

  assert!(invocation.is_stashing());
  invocation.invoke(&mut counter);
  assert_eq!(counter.calls, 1);
}

#[test]
fn pending_handler_invocation_invoke_async() {
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_i32);
  let repr = PersistentRepr::new("pid-1", 1, payload);
  let mut counter = Counter { calls: 0 };

  let invocation = PendingHandlerInvocation::async_handler(repr, |actor: &mut Counter, _| {
    actor.calls += 1;
  });

  assert!(!invocation.is_stashing());
  invocation.invoke(&mut counter);
  assert_eq!(counter.calls, 1);
}
