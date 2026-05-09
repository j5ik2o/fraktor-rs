use core::{hint::spin_loop, time::Duration};

use fraktor_actor_core_kernel_rs::actor::{messaging::AskError, setup::ActorSystemConfig};

use crate::{
  Behavior, TypedActorRef, TypedActorSystem, TypedProps,
  dsl::{AskPattern, Behaviors, StatusReply, TypedAskError},
};

#[derive(Clone)]
enum AskPatternCommand {
  Echo { value: u32, reply_to: TypedActorRef<u32> },
  Status { value: u32, reply_to: TypedActorRef<StatusReply<u32>> },
  Ignore { _reply_to: TypedActorRef<u32> },
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  panic!("condition not met");
}

fn ask_pattern_behavior() -> Behavior<AskPatternCommand> {
  Behaviors::receive_message(|_ctx, message| {
    match message {
      | AskPatternCommand::Echo { value, reply_to } => {
        let mut reply_to = reply_to.clone();
        reply_to.tell(*value);
      },
      | AskPatternCommand::Status { value, reply_to } => {
        let mut reply_to = reply_to.clone();
        reply_to.tell(StatusReply::success(*value));
      },
      | AskPatternCommand::Ignore { .. } => {},
    }
    Ok(Behaviors::same())
  })
}

#[test]
fn ask_pattern_exposes_timeout_aware_standalone_ask() {
  let props = TypedProps::<AskPatternCommand>::from_behavior_factory(ask_pattern_behavior);
  let system = TypedActorSystem::<AskPatternCommand>::create_from_props(
    &props,
    ActorSystemConfig::new(crate::test_support::test_tick_driver()),
  )
  .expect("system");
  let mut actor = system.user_guardian_ref();

  let response =
    AskPattern::ask(&mut actor, |reply_to| AskPatternCommand::Echo { value: 41, reply_to }, Duration::from_secs(1));
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());

  assert_eq!(future.try_take().expect("ready").expect("ok"), 41);

  system.terminate().expect("terminate");
}

#[test]
fn ask_pattern_exposes_timeout_aware_status_ask() {
  let props = TypedProps::<AskPatternCommand>::from_behavior_factory(ask_pattern_behavior);
  let system = TypedActorSystem::<AskPatternCommand>::create_from_props(
    &props,
    ActorSystemConfig::new(crate::test_support::test_tick_driver()),
  )
  .expect("system");
  let mut actor = system.user_guardian_ref();

  let response = AskPattern::ask_with_status(
    &mut actor,
    |reply_to| AskPatternCommand::Status { value: 7, reply_to },
    Duration::from_secs(1),
  );
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());

  let reply = future.try_take().expect("ready").expect("ok");
  assert_eq!(reply.into_result().expect("status ok"), 7);

  system.terminate().expect("terminate");
}

#[test]
fn ask_pattern_times_out_when_target_does_not_reply() {
  let props = TypedProps::<AskPatternCommand>::from_behavior_factory(ask_pattern_behavior);
  let system = TypedActorSystem::<AskPatternCommand>::create_from_props(
    &props,
    ActorSystemConfig::new(crate::test_support::test_tick_driver()),
  )
  .expect("system");
  let mut actor = system.user_guardian_ref();

  let response =
    AskPattern::ask(&mut actor, |reply_to| AskPatternCommand::Ignore { _reply_to: reply_to }, Duration::ZERO);
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());

  assert!(matches!(future.try_take().expect("ready"), Err(TypedAskError::AskFailed(AskError::Timeout))));

  system.terminate().expect("terminate");
}
