#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::{String, ToString};

use cellactor_actor_core_rs::{
  Actor, ActorContext, ActorError, ActorRef, ActorSystem, AnyMessage, AnyMessageView, Props,
};

struct Start;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let pong =
        ctx.spawn_child(&Props::from_fn(pong_factory)).map_err(|_| ActorError::recoverable("failed to spawn pong"))?;
      let ping =
        ctx.spawn_child(&Props::from_fn(ping_factory)).map_err(|_| ActorError::recoverable("failed to spawn ping"))?;
      let start_ping = StartPing { target: pong, reply: ping.clone(), count: 3 };
      ping.tell(AnyMessage::new(start_ping)).map_err(|_| ActorError::recoverable("failed to kick ping"))?;
    }
    Ok(())
  }
}

struct PingActor {
  awaiting: u32,
}

struct StartPing {
  target: ActorRef,
  reply:  ActorRef,
  count:  u32,
}

struct PingMessage {
  text: String,
}

struct PongReply {
  text: String,
}

impl PingActor {
  fn new() -> Self {
    Self { awaiting: 0 }
  }
}

impl Actor for PingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(cmd) = message.downcast_ref::<StartPing>() {
      self.awaiting = cmd.count;
      for index in 0..cmd.count {
        let payload = PingMessage { text: format_message(index) };
        let envelope = AnyMessage::new(payload).with_reply_to(cmd.reply.clone());
        cmd.target.tell(envelope).map_err(|_| ActorError::recoverable("ping send failed"))?;
      }
    } else if let Some(reply) = message.downcast_ref::<PongReply>() {
      if self.awaiting > 0 {
        self.awaiting -= 1;
      }
      #[cfg(feature = "std")]
      {
        use std::println;
        println!("pong replied: {}", reply.text);
      }
    }
    Ok(())
  }
}

struct PongActor;

impl Actor for PongActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(ping) = message.downcast_ref::<PingMessage>() {
      #[cfg(feature = "std")]
      {
        use std::println;
        println!("received ping: {}", ping.text);
      }
      let response = PongReply { text: ping.text.clone() };
      ctx.reply(AnyMessage::new(response)).map_err(|_| ActorError::recoverable("reply failed"))?;
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

fn guardian_factory() -> GuardianActor {
  GuardianActor
}

fn ping_factory() -> PingActor {
  PingActor::new()
}

fn pong_factory() -> PongActor {
  PongActor
}

#[cfg(feature = "std")]
fn main() {
  let system = ActorSystem::new(Props::from_fn(guardian_factory)).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
}

#[cfg(not(feature = "std"))]
fn main() {}
