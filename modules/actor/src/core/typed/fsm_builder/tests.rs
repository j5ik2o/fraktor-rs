use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use super::FsmBuilder;
use crate::core::{
  actor::ActorContextGeneric,
  system::ActorSystemGeneric,
  typed::{
    actor::{TypedActor, TypedActorContextGeneric},
    behavior_runner::BehaviorRunner,
    message_adapter::MessageAdapterRegistry,
  },
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProbeState {
  Idle,
  Active,
}

#[derive(Clone, Copy)]
enum ProbeMessage {
  Advance,
}

fn build_context() -> (ActorContextGeneric<'static, NoStdToolbox>, MessageAdapterRegistry<ProbeMessage, NoStdToolbox>) {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let ctx = ActorContextGeneric::new(&system, pid);
  (ctx, MessageAdapterRegistry::new())
}

#[test]
fn fsm_builder_transitions_between_states() {
  let calls = Arc::new(AtomicUsize::new(0));
  let calls_for_idle = Arc::clone(&calls);
  let calls_for_active = Arc::clone(&calls);

  let behavior = FsmBuilder::<ProbeState, ProbeMessage>::new(ProbeState::Idle)
    .when(ProbeState::Idle, move |message| match message {
      | ProbeMessage::Advance => {
        calls_for_idle.fetch_add(10, Ordering::SeqCst);
        Some(ProbeState::Active)
      },
    })
    .when(ProbeState::Active, move |message| match message {
      | ProbeMessage::Advance => {
        calls_for_active.fetch_add(1, Ordering::SeqCst);
        None
      },
    })
    .build();

  let mut runner = BehaviorRunner::new(behavior);
  let (mut ctx, mut registry) = build_context();
  let mut typed_ctx = TypedActorContextGeneric::from_untyped(&mut ctx, Some(&mut registry));

  runner.pre_start(&mut typed_ctx).expect("pre_start");
  runner.receive(&mut typed_ctx, &ProbeMessage::Advance).expect("first message");
  runner.receive(&mut typed_ctx, &ProbeMessage::Advance).expect("second message");

  assert_eq!(calls.load(Ordering::SeqCst), 11);
}
