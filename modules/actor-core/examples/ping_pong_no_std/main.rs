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

      let start_ping = StartPing { target: pong.actor_ref().clone(), reply_to: ctx.self_ref(), count: 3 };
      ping.tell(AnyMessage::new(start_ping)).map_err(|_| ActorError::recoverable("failed to start ping actor"))?;
    } else if let Some(reply) = message.downcast_ref::<PongReply>() {
      #[cfg(feature = "std")]
      {
        use std::println;
        println!("[{:?}] pong replied: {}", std::thread::current().id(), reply.text);
      }
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
      #[cfg(feature = "std")]
      {
        use std::println;
        println!("[{:?}] received ping: {}", std::thread::current().id(), ping.text);
      }
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

fn guardian_factory() -> GuardianActor {
  GuardianActor
}

fn ping_factory() -> PingActor {
  PingActor
}

fn pong_factory() -> PongActor {
  PongActor
}

#[cfg(feature = "std")]
fn main() {
  let props = Props::from_fn(guardian_factory);
  let system = ActorSystem::new(&props).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
  system.terminate().expect("terminate");
  system.run_until_terminated();
}

#[cfg(not(feature = "std"))]
fn main() {}
