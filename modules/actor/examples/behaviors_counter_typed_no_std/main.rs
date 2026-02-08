//! Functional behavior-based counter for no_std environments.
//!
//! Same counter pattern as `behaviors_counter_typed_std` but using only
//! core APIs without standard library dependencies.

#![cfg_attr(all(not(test), target_os = "none"), no_std)]

use fraktor_utils_rs::core::sync::sync_mutex_like::SyncMutexLike as _;
use fraktor_actor_rs::core::{
  error::ActorError,
  typed::{Behavior, Behaviors, TypedActorRef, TypedActorSystem, TypedProps},
};

#[derive(Clone)]
enum CounterCommand {
  Add(i32),
  Read { reply_to: TypedActorRef<i32> },
}

fn counter(total: i32) -> Behavior<CounterCommand> {
  Behaviors::receive_message(move |_ctx, message| match message {
    | CounterCommand::Add(delta) => Ok(counter(total + delta)),
    | CounterCommand::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
  })
}

#[cfg(not(target_os = "none"))]
#[allow(clippy::print_stdout)]
fn main() {
  use std::thread;

  // 開発環境では `cargo run --example typed_behaviors_counter` で実行し、ログで結果を確認する。
  let props = TypedProps::from_behavior_factory(|| counter(0));
  let system = TypedActorSystem::new(&props).expect("system");
  let counter = system.user_guardian_ref();
  let termination = system.when_terminated();

  counter.tell(CounterCommand::Add(4)).expect("add first");
  counter.tell(CounterCommand::Add(6)).expect("add second");

  let response = counter.ask::<i32, _>(|reply_to| CounterCommand::Read { reply_to }).expect("ask read");
  let future = response.future().clone();
  while !future.lock().is_ready() {
    thread::yield_now();
  }
  if let Some(result) = future.try_take() {
    match result {
      | Ok(value) => println!("typed behaviors counter result: {value}"),
      | Err(error) => println!("typed behaviors counter error: {error}"),
    }
  }

  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
