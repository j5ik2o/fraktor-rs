#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::{format, string::String};

use fraktor_actor_core_rs::{
  actor_prim::{Actor, ActorContext},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  system::ActorSystem,
};

#[derive(Clone)]
struct FetchRequest {
  id:   u32,
  path: String,
}

#[derive(Clone)]
struct FetchResult {
  id:      u32,
  payload: String,
}

struct FetchActor;

impl Actor for FetchActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(request) = message.downcast_ref::<FetchRequest>() {
      #[cfg(not(target_os = "none"))]
      println!("untyped fetch: start {}", request.path);
      let future = {
        let path = request.path.clone();
        let id = request.id;
        async move {
          let payload = fake_http_call(path.as_ref()).await;
          FetchResult { id, payload }
        }
      };
      ctx.pipe_to_self(future, AnyMessage::new).map_err(|error| ActorError::recoverable(error.to_string()))?;
    } else if let Some(result) = message.downcast_ref::<FetchResult>() {
      #[cfg(not(target_os = "none"))]
      println!("untyped fetch: completed id={} body={}", result.id, result.payload);
      ctx.system().terminate().map_err(|error| ActorError::from_send_error(&error))?;
      ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))?;
    }
    Ok(())
  }
}

async fn fake_http_call(path: &str) -> String {
  format!("response from {path}")
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let props = Props::from_fn(|| FetchActor);
  let system = ActorSystem::new(&props).expect("system");
  let termination = system.when_terminated();

  system
    .user_guardian_ref()
    .tell(AnyMessage::new(FetchRequest { id: 1, path: String::from("/users") }))
    .expect("fetch");

  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
