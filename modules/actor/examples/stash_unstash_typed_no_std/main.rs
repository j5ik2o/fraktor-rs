//! Demonstrates `StashBuffer::unstash` with a wrapping function.

#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use fraktor_actor_rs::core::{
  error::ActorError,
  typed::{Behavior, Behaviors, StashBuffer, TypedActorSystem, TypedProps, actor::TypedActorRef},
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
  Behaviors::receive_message(move |ctx, message| match message {
    | Command::Buffer(_) => {
      stash.stash(ctx)?;
      Ok(Behaviors::same())
    },
    | Command::Open => {
      let unstashed = stash.unstash(ctx, 2, |message| match message {
        | Command::Buffer(value) => Command::Buffer(value + 100),
        | other => other,
      })?;
      let _ = unstashed;
      Ok(open(total))
    },
    | Command::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
  })
}

fn open(total: i32) -> Behavior<Command> {
  Behaviors::receive_message(move |_ctx, message| match message {
    | Command::Buffer(value) => Ok(open(total + value)),
    | Command::Open => Ok(Behaviors::same()),
    | Command::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
  })
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::{thread, time::Duration};

  let props = TypedProps::from_behavior_factory(|| buffering(0));
  let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");
  let mut actor = system.user_guardian_ref();

  actor.tell(Command::Buffer(5)).expect("buffer one");
  actor.tell(Command::Buffer(3)).expect("buffer two");
  actor.tell(Command::Open).expect("open");

  let response = actor.ask::<i32, _>(|reply_to| Command::Read { reply_to }).expect("ask");
  let mut future = response.future().clone();
  while !future.is_ready() {
    thread::sleep(Duration::from_millis(10));
  }
  let value = future.try_take().expect("ready").expect("ok");
  println!("unstashed total = {value}");

  system.terminate().expect("terminate");
}

#[cfg(target_os = "none")]
fn main() {}
