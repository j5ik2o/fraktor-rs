use core::time::Duration;
use std::{thread, time::Instant};

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::actor::setup::ActorSystemConfig;
use fraktor_actor_typed_rs::{Behavior, TypedActorRef, TypedActorSystem, dsl::Behaviors};

#[derive(Clone)]
enum Command {
  Question { reply_to: TypedActorRef<u32> },
}

fn responder() -> Behavior<Command> {
  Behaviors::receive_message(|_ctx, message: &Command| {
    let Command::Question { reply_to } = message;
    let mut reply_to = reply_to.clone();
    reply_to.tell(42);
    Ok(Behaviors::same())
  })
}

fn main() {
  let system =
    TypedActorSystem::create_from_behavior_factory(responder, ActorSystemConfig::new(StdTickDriver::default()))
      .expect("system");
  let termination = system.when_terminated();
  let mut responder = system.user_guardian_ref();

  let response = responder.ask::<u32, _>(|reply_to| Command::Question { reply_to });
  let mut future = response.future().clone();
  let deadline = Instant::now() + Duration::from_secs(1);
  while !future.is_ready() {
    assert!(Instant::now() < deadline, "ask should complete within 1 second");
    thread::sleep(Duration::from_millis(1));
  }
  let value = future.try_take().expect("ready").expect("ok");
  assert_eq!(value, 42);
  println!("typed_interaction_patterns received response: {value}");

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
