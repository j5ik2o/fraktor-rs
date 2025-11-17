use alloc::{string::String, sync::Arc};
use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_utils_core_rs::core::runtime_toolbox::NoStdToolbox;

use super::BehaviorRunner;
use crate::core::{
  actor_prim::ActorContextGeneric,
  system::ActorSystemGeneric,
  typed::{
    Behaviors,
    actor_prim::{TypedActor, TypedActorContextGeneric},
    behavior_signal::BehaviorSignal,
    message_adapter::{AdapterFailure, MessageAdapterRegistry},
  },
};

struct ProbeMessage;

fn build_context() -> (ActorContextGeneric<'static, NoStdToolbox>, MessageAdapterRegistry<ProbeMessage, NoStdToolbox>) {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let ctx = ActorContextGeneric::new(&system, pid);
  (ctx, MessageAdapterRegistry::new())
}

#[test]
fn behavior_runner_escalates_without_signal_handler() {
  let behavior = Behaviors::receive_message(|_, _msg: &ProbeMessage| Ok(Behaviors::same()));
  let mut runner = BehaviorRunner::new(behavior);
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.on_adapter_failure(&mut typed_ctx, AdapterFailure::Custom(String::from("boom")));
  assert!(result.is_err());
}

#[test]
fn behavior_runner_allows_handled_adapter_failure() {
  let handled = Arc::new(AtomicBool::new(false));
  let witness = handled.clone();
  let behavior = Behaviors::receive_signal(move |_, signal| {
    if matches!(signal, BehaviorSignal::AdapterFailed(_)) {
      witness.store(true, Ordering::SeqCst);
    }
    Ok(Behaviors::same())
  });
  let mut runner = BehaviorRunner::new(behavior);
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.on_adapter_failure(&mut typed_ctx, AdapterFailure::Custom(String::from("oops")));
  assert!(result.is_ok());
  assert!(handled.load(Ordering::SeqCst));
}
