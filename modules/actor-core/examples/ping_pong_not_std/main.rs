#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::string::{String, ToString};

use fraktor_actor_core_rs::{
  actor_prim::{Actor, ActorContext, actor_ref::ActorRef},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  system::ActorSystem,
};

struct Start;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let pong =
        ctx.spawn_child(&Props::from_fn(|| PongActor)).map_err(|_| ActorError::recoverable("failed to spawn pong"))?;
      let ping =
        ctx.spawn_child(&Props::from_fn(|| PingActor)).map_err(|_| ActorError::recoverable("failed to spawn ping"))?;

      let start_ping = StartPing { target: pong.actor_ref().clone(), reply_to: ctx.self_ref(), count: 3 };
      ping.tell(AnyMessage::new(start_ping)).map_err(|_| ActorError::recoverable("failed to start ping actor"))?;
    } else if let Some(reply) = message.downcast_ref::<PongReply>() {
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] pong replied: {}", std::thread::current().id(), reply.text);
    }
    Ok(())
  }
}

struct StartPing {
  target:   ActorRef,
  reply_to: ActorRef,
  count:    u32,
}

struct PingMessage {
  text:     String,
  reply_to: ActorRef,
}

struct PongReply {
  text: String,
}

struct PingActor;

impl Actor for PingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(cmd) = message.downcast_ref::<StartPing>() {
      for index in 0..cmd.count {
        let payload = PingMessage { text: format_message(index), reply_to: cmd.reply_to.clone() };
        cmd.target.tell(AnyMessage::new(payload)).map_err(|_| ActorError::recoverable("failed to send ping"))?;
      }
    }
    Ok(())
  }
}

struct PongActor;

impl Actor for PongActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(ping) = message.downcast_ref::<PingMessage>() {
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] received ping: {}", std::thread::current().id(), ping.text);

      let response = PongReply { text: ping.text.clone() };
      ping.reply_to.tell(AnyMessage::new(response)).map_err(|_| ActorError::recoverable("reply failed"))?;
    }
    Ok(())
  }
}

fn format_message(index: u32) -> String {
  let number = index + 1;
  let mut out = String::from("ping-");
  out.push_str(&number.to_string());
  out
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let props = Props::from_fn(|| GuardianActor);
  let system = ActorSystem::new(&props).expect("system");
  let termination = system.when_terminated();
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
