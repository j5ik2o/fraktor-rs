//! Demonstrates `Behaviors::receive_message_partial()` for partial message handling (no_std
//! version).
//!
//! The actor handles only some message variants; unmatched variants are
//! automatically treated as unhandled. This mirrors Pekko's
//! `Behaviors.receiveMessagePartial`.

#![cfg_attr(all(not(test), target_os = "none"), no_std)]

#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use fraktor_actor_rs::core::typed::{Behavior, Behaviors, TypedActorSystem, TypedProps};
use fraktor_utils_rs::core::sync::SharedAccess;

#[derive(Clone)]
enum Command {
  Greet,
  Farewell,
  Unknown,
}

fn partial_behavior() -> Behavior<Command> {
  // Only Greet and Farewell are handled; Unknown returns None → unhandled.
  Behaviors::receive_message_partial(|_ctx, msg: &Command| match msg {
    | Command::Greet => {
      #[cfg(not(target_os = "none"))]
      println!("Hello!");
      Ok(Some(Behaviors::same()))
    },
    | Command::Farewell => {
      #[cfg(not(target_os = "none"))]
      println!("Goodbye!");
      Ok(Some(Behaviors::same()))
    },
    | Command::Unknown => {
      #[cfg(not(target_os = "none"))]
      println!("(unhandled message)");
      Ok(None)
    },
  })
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let props = TypedProps::from_behavior_factory(partial_behavior);
  let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");
  let termination = system.as_untyped().when_terminated();

  let mut actor = system.user_guardian_ref();
  actor.tell(Command::Greet).expect("greet");
  actor.tell(Command::Unknown).expect("unknown");
  actor.tell(Command::Farewell).expect("farewell");

  system.terminate().expect("terminate");
  while !termination.with_read(|af| af.is_ready()) {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
