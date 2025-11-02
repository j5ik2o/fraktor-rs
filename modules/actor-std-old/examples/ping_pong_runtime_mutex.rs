//! Ping-pong example demonstrating that `ActorRuntimeMutex` maps to `StdSyncMutex`
//! when the `std` feature is enabled. The guardian actor keeps a log protected by
//! `ActorRuntimeMutex`, which resolves to `std::sync::Mutex` in this build.

use std::{any::type_name, time::Duration};

use cellactor_actor_std_rs::{
  Actor, ActorContext, ActorError, ActorRef, ActorRuntimeMutex, ActorSystem, AnyMessage, AnyMessageView, Props,
  TokioPropsExt,
};
use cellactor_utils_core_rs::sync::ArcShared;

struct Start {
  rounds: u32,
}

struct StartPing {
  target:   ActorRef,
  reply_to: ActorRef,
  rounds:   u32,
}

struct PingShot {
  index:    u32,
  reply_to: ActorRef,
}

struct PongReply {
  text: String,
}

struct GuardianActor {
  log:      ArcShared<ActorRuntimeMutex<Vec<String>>>,
  expected: u32,
}

impl GuardianActor {
  fn new(log: ArcShared<ActorRuntimeMutex<Vec<String>>>) -> Self {
    Self { log, expected: 0 }
  }
}

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(start) = message.downcast_ref::<Start>() {
      println!("ActorRuntimeMutex backend: {}", type_name::<ActorRuntimeMutex<Vec<String>>>());

      let pong_props = Props::from_fn(|| PongActor).with_tokio_dispatcher_current();
      let pong = ctx.spawn_child(&pong_props).map_err(|_| ActorError::recoverable("spawn pong"))?;

      let ping_props = Props::from_fn(|| PingActor).with_tokio_dispatcher_current();
      let ping = ctx.spawn_child(&ping_props).map_err(|_| ActorError::recoverable("spawn ping"))?;

      self.expected = start.rounds;
      let launch = StartPing { target: pong.actor_ref().clone(), reply_to: ctx.self_ref(), rounds: start.rounds };
      ping.tell(AnyMessage::new(launch)).map_err(|_| ActorError::recoverable("launch ping"))?;
    } else if let Some(reply) = message.downcast_ref::<PongReply>() {
      {
        let mut guard = self.log.lock();
        guard.push(reply.text.clone());
        println!("guardian logged reply: {}", reply.text);
        if guard.len() as u32 == self.expected {
          println!("guardian collected {} replies", self.expected);
        }
      }
    }
    Ok(())
  }
}

struct PingActor;

impl Actor for PingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(start) = message.downcast_ref::<StartPing>() {
      for index in 0..start.rounds {
        let shot = PingShot { index, reply_to: start.reply_to.clone() };
        start.target.tell(AnyMessage::new(shot)).map_err(|_| ActorError::recoverable("send ping"))?;
      }
    }
    Ok(())
  }
}

struct PongActor;

impl Actor for PongActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(ping) = message.downcast_ref::<PingShot>() {
      let reply = PongReply { text: format!("pong-{}", ping.index + 1) };
      ping.reply_to.tell(AnyMessage::new(reply)).map_err(|_| ActorError::recoverable("reply pong"))?;
    }
    Ok(())
  }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
  let log = ArcShared::new(ActorRuntimeMutex::new(Vec::new()));

  let guardian_props = Props::from_fn({
    let log = log.clone();
    move || GuardianActor::new(log.clone())
  })
  .with_tokio_dispatcher_current();

  let system = ActorSystem::new(&guardian_props).expect("system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(Start { rounds: 3 })).expect("start guardian");

  tokio::time::sleep(Duration::from_millis(100)).await;

  system.terminate().expect("terminate");
  termination.listener().await;

  let entries = log.lock().clone();
  println!("guardian log entries: {entries:?}");
}
