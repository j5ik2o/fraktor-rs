#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{boxed::Box, string::{String, ToString}};

use actor_core::{Actor, ActorContext, ActorError, ActorRef, ActorSystem, AnyMessage, AnyOwnedMessage, Props};

struct Start;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, msg: AnyMessage<'_>) -> Result<(), ActorError> {
    if msg.downcast_ref::<Start>().is_some() {
      let pong = ctx.spawn_child(Props::from_fn(pong_factory))?;
      let ping = ctx.spawn_child(Props::from_fn(ping_factory))?;
      let start_ping = StartPing { target: pong, count: 3 };
      ping.tell(AnyOwnedMessage::new(start_ping))?;
    }
    Ok(())
  }
}

struct PingActor;

struct StartPing {
  target: ActorRef,
  count:  u32,
}

impl Actor for PingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, msg: AnyMessage<'_>) -> Result<(), ActorError> {
    if let Some(cmd) = msg.downcast_ref::<StartPing>() {
      for index in 0..cmd.count {
        let text = format_message(index);
        let _ = cmd.target.tell(AnyOwnedMessage::new(text.clone()));
      }
    }
    Ok(())
  }
}

struct PongActor;

impl Actor for PongActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, msg: AnyMessage<'_>) -> Result<(), ActorError> {
    if let Some(text) = msg.downcast_ref::<String>() {
      #[cfg(feature = "std")]
      {
        use std::println;
        println!("received: {text}");
      }
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
  let system = ActorSystem::new(Props::from_fn(guardian_factory)).expect("system");
  system
    .user_guardian_ref()
    .tell(AnyOwnedMessage::new(Start))
    .expect("start");
}

#[cfg(not(feature = "std"))]
fn main() {}
