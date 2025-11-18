#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use alloc::string::{String, ToString};

use fraktor_actor_rs::core::{
  error::ActorError,
  typed::{
    TypedActorSystem, TypedProps,
    actor_prim::{TypedActor, TypedActorContext, TypedActorRef},
  },
};

#[derive(Clone)]
struct PongReply {
  text: String,
}

enum GuardianCommand {
  Start,
  PongNotified(PongReply),
}

struct GuardianActor;

impl TypedActor<GuardianCommand> for GuardianActor {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, GuardianCommand>,
    message: &GuardianCommand,
  ) -> Result<(), ActorError> {
    match message {
      | GuardianCommand::Start => {
        let pong_ref = ctx
          .spawn_child(&TypedProps::new(|| PongActor))
          .map_err(|_| ActorError::recoverable("failed to spawn pong"))?
          .actor_ref();
        let ping_ref = ctx
          .spawn_child(&TypedProps::new(|| PingActor))
          .map_err(|_| ActorError::recoverable("failed to spawn ping"))?
          .actor_ref();

        let start = StartPing { target: pong_ref, reply_to: ctx.self_ref(), count: 3 };
        ping_ref.tell(PingCommand::Start(start)).map_err(|_| ActorError::recoverable("failed to start ping"))?;
      },
      | GuardianCommand::PongNotified(reply) => {
        #[cfg(not(target_os = "none"))]
        println!("[{:?}] pong replied: {}", std::thread::current().id(), reply.text);
      },
    }
    Ok(())
  }
}

#[derive(Clone)]
struct StartPing {
  target:   TypedActorRef<PongCommand>,
  reply_to: TypedActorRef<GuardianCommand>,
  count:    u32,
}

enum PingCommand {
  Start(StartPing),
}

struct PingActor;

impl TypedActor<PingCommand> for PingActor {
  fn receive(
    &mut self,
    _ctx: &mut TypedActorContext<'_, PingCommand>,
    message: &PingCommand,
  ) -> Result<(), ActorError> {
    match message {
      | PingCommand::Start(cmd) => {
        for index in 0..cmd.count {
          let payload =
            PongCommand::Ping(PingMessage { text: format_message(index), reply_to: cmd.reply_to.clone() });
          cmd.target.tell(payload).map_err(|_| ActorError::recoverable("failed to send ping"))?;
        }
      },
    }
    Ok(())
  }
}

#[derive(Clone)]
struct PingMessage {
  text:     String,
  reply_to: TypedActorRef<GuardianCommand>,
}

enum PongCommand {
  Ping(PingMessage),
}

struct PongActor;

impl TypedActor<PongCommand> for PongActor {
  fn receive(
    &mut self,
    _ctx: &mut TypedActorContext<'_, PongCommand>,
    message: &PongCommand,
  ) -> Result<(), ActorError> {
    match message {
      | PongCommand::Ping(ping) => {
        #[cfg(not(target_os = "none"))]
        println!("[{:?}] received ping: {}", std::thread::current().id(), ping.text);

        let response = GuardianCommand::PongNotified(PongReply { text: ping.text.clone() });
        ping.reply_to.tell(response).map_err(|_| ActorError::recoverable("reply failed"))?;
      },
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

  let props = TypedProps::new(|| GuardianActor);
  let tick_driver = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");
  let termination = system.as_untyped().when_terminated();
  system.user_guardian_ref().tell(GuardianCommand::Start).expect("start");
  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
