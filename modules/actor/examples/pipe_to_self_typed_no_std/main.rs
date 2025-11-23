#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use alloc::{
  format,
  string::{String, ToString},
};

use fraktor_actor_rs::core::{
  error::ActorError,
  typed::{
    AdapterFailure, TypedActorSystem, TypedProps,
    actor_prim::{TypedActor, TypedActorContext},
  },
};

#[derive(Clone)]
enum FetchCommand {
  Start(String),
  Completed(String),
  Failed(String),
}

struct FetchClient;

impl TypedActor<FetchCommand> for FetchClient {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, '_, FetchCommand>,
    message: &FetchCommand,
  ) -> Result<(), ActorError> {
    match message {
      | FetchCommand::Start(path) => {
        #[cfg(not(target_os = "none"))]
        println!("typed client: fetch {}", path);
        let request_path = path.clone();
        ctx
          .pipe_to_self(
            async move {
              fake_http_call(request_path.as_ref()).await.map_err(|error| AdapterFailure::Custom(error.to_string()))
            },
            |body| Ok(FetchCommand::Completed(body)),
            |failure| Ok(FetchCommand::Failed(format!("{:?}", failure))),
          )
          .map_err(|error| ActorError::recoverable(error.to_string()))?;
      },
      | FetchCommand::Completed(body) => {
        #[cfg(not(target_os = "none"))]
        println!("typed client: completed body={body}");
        ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))?;
      },
      | FetchCommand::Failed(reason) => {
        #[cfg(not(target_os = "none"))]
        println!("typed client: failed {reason}");
        ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))?;
      },
    }
    Ok(())
  }
}

async fn fake_http_call(path: &str) -> Result<String, &'static str> {
  if path.is_empty() { Err("path missing") } else { Ok(format!("typed response from {path}")) }
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let props = TypedProps::new(|| FetchClient);
  let tick_driver = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");

  let actor = system.user_guardian_ref();
  actor.tell(FetchCommand::Start(String::from("/posts"))).expect("start");

  let termination = system.as_untyped().when_terminated();
  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
