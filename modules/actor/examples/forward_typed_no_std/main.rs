//! Demonstrates `TypedActorContext::forward()` for typed actor message forwarding (no_std version).
//!
//! A router actor receives messages and forwards them to a worker actor,
//! preserving the original sender. This mirrors Pekko's `ActorRef.forward`.

#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use fraktor_actor_rs::core::{
  error::ActorError,
  typed::{Behavior, Behaviors, TypedActorSystem, TypedProps},
};
use fraktor_utils_rs::core::sync::SharedAccess;

#[derive(Clone)]
struct Payload(u32);

fn router_behavior() -> Behavior<Payload> {
  Behaviors::setup(|ctx| {
    let worker =
      ctx.spawn_child(&TypedProps::from_behavior_factory(worker_behavior)).expect("spawn worker").actor_ref();

    Behaviors::receive_message(move |ctx, msg: &Payload| {
      // forward() preserves the original sender, unlike tell().
      ctx.forward(&worker, msg.clone()).map_err(|e| ActorError::from_send_error(&e))?;
      Ok(Behaviors::same())
    })
  })
}

fn worker_behavior() -> Behavior<Payload> {
  Behaviors::receive_message(|_ctx, message: &Payload| {
    #[cfg(not(target_os = "none"))]
    println!("worker received forwarded payload: {}", message.0);
    Ok(Behaviors::same())
  })
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let props = TypedProps::from_behavior_factory(router_behavior);
  let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");
  let termination = system.as_untyped().when_terminated();

  system.user_guardian_ref().tell(Payload(42)).expect("tell router");

  system.terminate().expect("terminate");
  while !termination.with_read(|af| af.is_ready()) {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
