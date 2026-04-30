#![cfg(not(target_os = "none"))]

use core::time::Duration;
use std::{thread, time::Instant};

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::{
  kernel::actor::setup::ActorSystemConfig,
  typed::{
    Behavior, TypedActorRef, TypedActorSystem, TypedProps,
    dsl::{Behaviors, StashBuffer},
  },
};

#[derive(Clone)]
enum Command {
  Buffer(i32),
  Open,
  Read { reply_to: TypedActorRef<i32> },
}

fn buffering(total: i32) -> Behavior<Command> {
  Behaviors::with_stash(8, move |stash| closed(total, stash))
}

fn closed(total: i32, stash: StashBuffer<Command>) -> Behavior<Command> {
  Behaviors::receive_message(move |ctx, message: &Command| match message {
    | Command::Buffer(_) => {
      stash.stash(ctx)?;
      Ok(Behaviors::same())
    },
    | Command::Open => {
      let replayed = stash.unstash(ctx, 2, |message| match message {
        | Command::Buffer(value) => Command::Buffer(value + 100),
        | other => other,
      })?;
      debug_assert_eq!(replayed, 2);
      Ok(open(total))
    },
    | Command::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total);
      Ok(Behaviors::same())
    },
  })
}

fn open(total: i32) -> Behavior<Command> {
  Behaviors::receive_message(move |_ctx, message: &Command| match message {
    | Command::Buffer(value) => Ok(open(total + value)),
    | Command::Open => Ok(Behaviors::same()),
    | Command::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total);
      Ok(Behaviors::same())
    },
  })
}

fn main() {
  let props = TypedProps::from_behavior_factory(|| buffering(0)).with_stash_mailbox();
  let system =
    TypedActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();
  let mut actor = system.user_guardian_ref();

  actor.tell(Command::Buffer(5));
  actor.tell(Command::Buffer(3));
  actor.tell(Command::Open);
  assert_eq!(read_total(&mut actor), 208);

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn read_total(actor: &mut TypedActorRef<Command>) -> i32 {
  let response = actor.ask::<i32, _>(|reply_to| Command::Read { reply_to });
  let mut future = response.future().clone();
  let deadline = Instant::now() + Duration::from_secs(1);
  while !future.is_ready() {
    assert!(Instant::now() < deadline, "stash read should complete");
    thread::sleep(Duration::from_millis(1));
  }
  future.try_take().expect("ready").expect("ok")
}
