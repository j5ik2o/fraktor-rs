//! Demonstrates `TypedActorContext::ask()` for request-response interaction (no_std version).
//!
//! A requester actor sends an ask-style request to a responder actor.
//! The response is piped back as a typed message with timeout and failure handling.
//! This mirrors Pekko's `pipeToSelf(target.ask(...))` pattern.

#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use alloc::format;
use core::time::Duration;

use fraktor_actor_rs::core::{
  error::ActorError,
  typed::{Behavior, Behaviors, TypedActorSystem, TypedProps, actor::TypedActorRef},
};
use fraktor_utils_rs::core::sync::SharedAccess;

#[derive(Clone)]
enum ResponderMsg {
  GetValue { reply_to: TypedActorRef<u32> },
}

#[derive(Clone)]
enum RequesterMsg {
  Start,
  GotResponse(u32),
  GotFailure,
}

fn responder_behavior() -> Behavior<ResponderMsg> {
  Behaviors::receive_message(|_ctx, msg: &ResponderMsg| match msg {
    | ResponderMsg::GetValue { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(42).map_err(|e| ActorError::from_send_error(&e))?;
      Ok(Behaviors::same())
    },
  })
}

fn requester_behavior() -> Behavior<RequesterMsg> {
  Behaviors::setup(|ctx| {
    let child = ctx.spawn_child(&TypedProps::from_behavior_factory(responder_behavior)).expect("spawn responder");
    let responder_ref = child.actor_ref();

    Behaviors::receive_message(move |ctx, msg: &RequesterMsg| match msg {
      | RequesterMsg::Start => {
        let mut target = responder_ref.clone();
        ctx
          .ask(
            &mut target,
            |reply_to| ResponderMsg::GetValue { reply_to },
            |result| match result {
              | Ok(value) => RequesterMsg::GotResponse(value),
              | Err(_) => RequesterMsg::GotFailure,
            },
            Duration::from_secs(5),
          )
          .map_err(|e| ActorError::recoverable(format!("ask failed: {e:?}")))?;
        Ok(Behaviors::same())
      },
      | RequesterMsg::GotResponse(value) => {
        #[cfg(not(target_os = "none"))]
        println!("received response: {value}");
        Ok(Behaviors::same())
      },
      | RequesterMsg::GotFailure => {
        #[cfg(not(target_os = "none"))]
        println!("ask failed (timeout or error)");
        Ok(Behaviors::same())
      },
    })
  })
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let props = TypedProps::from_behavior_factory(requester_behavior);
  let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");
  let termination = system.as_untyped().when_terminated();

  system.user_guardian_ref().tell(RequesterMsg::Start).expect("start");

  // Allow time for the ask round-trip.
  thread::sleep(std::time::Duration::from_millis(100));

  system.terminate().expect("terminate");
  while !termination.with_read(|af| af.is_ready()) {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
