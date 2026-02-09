//! Typed stash example that buffers messages until the actor transitions to open mode.

#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use fraktor_actor_rs::core::{
  error::ActorError,
  typed::{Behavior, Behaviors, StashBuffer, TypedActorSystem, TypedProps, actor::TypedActorRef},
};

enum StashCommand {
  Buffer(i32),
  Open,
  Read { reply_to: TypedActorRef<i32> },
}

fn stash_behavior(total: i32) -> Behavior<StashCommand> {
  Behaviors::with_stash(32, move |stash| locked(total, stash))
}

fn locked(total: i32, stash: StashBuffer<StashCommand>) -> Behavior<StashCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | StashCommand::Buffer(_) => {
      stash.stash(ctx)?;
      Ok(Behaviors::same())
    },
    | StashCommand::Open => {
      let _ = stash.unstash_all(ctx)?;
      Ok(open(total))
    },
    | StashCommand::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
  })
}

fn open(total: i32) -> Behavior<StashCommand> {
  Behaviors::receive_message(move |_ctx, message| match message {
    | StashCommand::Buffer(value) => Ok(open(total + value)),
    | StashCommand::Open => Ok(Behaviors::same()),
    | StashCommand::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
  })
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let props = TypedProps::from_behavior_factory(|| stash_behavior(0));
  let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");
  let mut actor = system.user_guardian_ref();

  actor.tell(StashCommand::Buffer(5)).expect("buffer one");
  actor.tell(StashCommand::Buffer(3)).expect("buffer two");
  actor.tell(StashCommand::Open).expect("open");

  let response = actor.ask::<i32, _>(|reply_to| StashCommand::Read { reply_to }).expect("ask");
  let mut future = response.future().clone();
  while !future.is_ready() {
    thread::yield_now();
  }
  let value = future.try_take().expect("result").expect("payload");
  println!("stashed total = {}", value);

  system.terminate().expect("terminate");
}

#[cfg(target_os = "none")]
fn main() {}
