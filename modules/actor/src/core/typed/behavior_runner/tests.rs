use alloc::{string::String, sync::Arc};
use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use super::BehaviorRunner;
use crate::core::{
  actor::ActorContextGeneric,
  error::ActorError,
  system::ActorSystemGeneric,
  typed::{
    Behaviors,
    actor::{TypedActor, TypedActorContextGeneric},
    behavior::Behavior,
    behavior_signal::BehaviorSignal,
    message_adapter::{AdapterError, MessageAdapterRegistry},
  },
};

struct ProbeMessage;

fn build_context() -> (ActorContextGeneric<'static, NoStdToolbox>, MessageAdapterRegistry<ProbeMessage, NoStdToolbox>) {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let ctx = ActorContextGeneric::new(&system, pid);
  (ctx, MessageAdapterRegistry::new())
}

fn build_context_with_pids(count: usize) -> (ActorSystemGeneric<NoStdToolbox>, Vec<crate::core::actor::Pid>) {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pids: Vec<_> = (0..count).map(|_| system.allocate_pid()).collect();
  (system, pids)
}

fn signal_probe_behavior(
  target_signal: fn(&BehaviorSignal) -> bool,
  witness: Arc<AtomicBool>,
) -> Behavior<ProbeMessage, NoStdToolbox> {
  Behaviors::receive_signal(move |_, signal| {
    if target_signal(signal) {
      witness.store(true, Ordering::SeqCst);
    }
    Ok(Behaviors::same())
  })
}

#[test]
fn behavior_runner_escalates_without_signal_handler() {
  let behavior = Behaviors::receive_message(|_, _msg: &ProbeMessage| Ok(Behaviors::same()));
  let mut runner = BehaviorRunner::new(behavior);
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.on_adapter_failure(&mut typed_ctx, AdapterError::Custom(String::from("boom")));
  assert!(result.is_err());
}

#[test]
fn behavior_runner_allows_handled_adapter_failure() {
  let handled = Arc::new(AtomicBool::new(false));
  let behavior = signal_probe_behavior(|s| matches!(s, BehaviorSignal::MessageAdaptionFailure(_)), handled.clone());
  let mut runner = BehaviorRunner::new(behavior);
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.on_adapter_failure(&mut typed_ctx, AdapterError::Custom(String::from("oops")));
  assert!(result.is_ok());
  assert!(handled.load(Ordering::SeqCst));
}

#[test]
fn behavior_runner_dispatches_pre_restart_signal() {
  let received = Arc::new(AtomicBool::new(false));
  let behavior = signal_probe_behavior(|s| matches!(s, BehaviorSignal::PreRestart), received.clone());
  let mut runner = BehaviorRunner::new(behavior);
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.pre_restart(&mut typed_ctx);
  assert!(result.is_ok());
  assert!(received.load(Ordering::SeqCst));
}

#[test]
fn behavior_runner_dispatches_child_failed_signal() {
  let received = Arc::new(AtomicBool::new(false));
  let behavior = signal_probe_behavior(|s| matches!(s, BehaviorSignal::ChildFailed { .. }), received.clone());
  let mut runner = BehaviorRunner::new(behavior);
  let (system, pids) = build_context_with_pids(2);
  let mut ctx = ActorContextGeneric::new(&system, pids[0]);
  let mut registry = MessageAdapterRegistry::<ProbeMessage, NoStdToolbox>::new();
  let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut ctx, Some(&mut registry));
  let error = ActorError::recoverable("child boom");
  let result = runner.on_child_failed(&mut typed_ctx, pids[1], &error);
  assert!(result.is_ok());
  assert!(received.load(Ordering::SeqCst));
}

#[test]
fn behavior_runner_death_pact_errors_without_signal_handler() {
  let behavior = Behaviors::receive_message(|_, _msg: &ProbeMessage| Ok(Behaviors::same()));
  let mut runner = BehaviorRunner::new(behavior);
  let (system, pids) = build_context_with_pids(2);
  let mut ctx = ActorContextGeneric::new(&system, pids[0]);
  let mut registry = MessageAdapterRegistry::<ProbeMessage, NoStdToolbox>::new();
  let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.on_terminated(&mut typed_ctx, pids[1]);
  assert!(result.is_err());
}

#[test]
fn behavior_runner_death_pact_succeeds_with_signal_handler() {
  let received = Arc::new(AtomicBool::new(false));
  let behavior = signal_probe_behavior(|s| matches!(s, BehaviorSignal::Terminated(_)), received.clone());
  let mut runner = BehaviorRunner::new(behavior);
  let (system, pids) = build_context_with_pids(2);
  let mut ctx = ActorContextGeneric::new(&system, pids[0]);
  let mut registry = MessageAdapterRegistry::<ProbeMessage, NoStdToolbox>::new();
  let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut ctx, Some(&mut registry));
  let result = runner.on_terminated(&mut typed_ctx, pids[1]);
  assert!(result.is_ok());
  assert!(received.load(Ordering::SeqCst));
}
