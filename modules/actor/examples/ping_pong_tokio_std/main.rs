//! Ping-pong messaging example on a Tokio multi-thread runtime.
//!
//! Demonstrates the untyped `Actor` API with `ActorSystem::quickstart()`
//! for Tokio-based scheduling and dispatcher defaults.

use std::{string::String, time::Duration};

use fraktor_actor_rs::{
  core::error::ActorError,
  std::{
    actor::{Actor, ActorContext, ActorRef},
    futures::ActorFutureListener,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    system::ActorSystem,
  },
};

struct Start;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let pong_props = Props::from_fn(|| PongActor);
      let pong = ctx.spawn_child(&pong_props).map_err(|_| ActorError::recoverable("failed to spawn pong"))?;

      let ping_props = Props::from_fn(|| PingActor);
      let mut ping = ctx.spawn_child(&ping_props).map_err(|_| ActorError::recoverable("failed to spawn ping"))?;

      let start_ping = StartPing { target: pong.actor_ref().clone(), reply_to: ctx.self_ref(), count: 3 };
      ping.tell(AnyMessage::new(start_ping)).map_err(|_| ActorError::recoverable("failed to start ping actor"))?;
    } else if let Some(reply) = message.downcast_ref::<PongReply>() {
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
  fn receive(&mut self, _ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(cmd) = message.downcast_ref::<StartPing>() {
      for index in 0..cmd.count {
        let payload = PingMessage { text: format!("ping-{}", index + 1), reply_to: cmd.reply_to.clone() };
        cmd
          .target
          .clone()
          .tell(AnyMessage::new(payload))
          .map_err(|_| ActorError::recoverable("failed to send ping"))?;
      }
    }
    Ok(())
  }
}

struct PongActor;

impl Actor for PongActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(ping) = message.downcast_ref::<PingMessage>() {
      println!("[{:?}] received ping: {}", std::thread::current().id(), ping.text);
      let response = PongReply { text: ping.text.clone() };
      ping.reply_to.clone().tell(AnyMessage::new(response)).map_err(|_| ActorError::recoverable("reply failed"))?;
    }
    Ok(())
  }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
  let props: Props = Props::from_fn(|| GuardianActor);
  let system = ActorSystem::quickstart(&props).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  tokio::time::sleep(Duration::from_millis(50)).await;

  system.terminate().expect("terminate");

  ActorFutureListener::new(system.when_terminated()).await;
}
