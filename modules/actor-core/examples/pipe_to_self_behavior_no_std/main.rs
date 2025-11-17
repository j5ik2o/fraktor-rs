#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::{format, string::String};

use fraktor_actor_core_rs::core::{
  error::ActorError,
  typed::{AdapterFailure, Behavior, Behaviors, TypedActorSystem, TypedProps},
};

#[derive(Clone)]
enum Command {
  Start(String),
  Completed(String),
  Failed(String),
}

fn fetch_behavior() -> Behavior<Command> {
  Behaviors::receive_message(|ctx, message| match message {
    | Command::Start(path) => {
      let request_path = path.clone();
      ctx
        .pipe_to_self(
          async move { fake_http_call(&request_path).await.map_err(|error| AdapterFailure::Custom(error.to_string())) },
          |body| Ok(Command::Completed(body)),
          |failure| Ok(Command::Failed(format!("{:?}", failure))),
        )
        .map_err(|error| ActorError::recoverable(error.to_string()))?;
      Ok(Behaviors::same())
    },
    | Command::Completed(body) => {
      #[cfg(not(target_os = "none"))]
      println!("behavior: completed {body}");
      ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
    | Command::Failed(reason) => {
      #[cfg(not(target_os = "none"))]
      println!("behavior: failed {reason}");
      ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
  })
}

async fn fake_http_call(path: &str) -> Result<String, &'static str> {
  Ok(format!("behavior response from {path}"))
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let props = TypedProps::from_behavior_factory(fetch_behavior);
  let system = TypedActorSystem::new(&props).expect("system");
  system.user_guardian_ref().tell(Command::Start(String::from("/behavior-start"))).expect("start");

  let termination = system.as_untyped().when_terminated();
  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
