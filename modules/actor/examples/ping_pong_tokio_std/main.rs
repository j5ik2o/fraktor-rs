use std::{string::String, time::Duration};

use fraktor_actor_rs::{
  core::error::ActorError,
  std::{
    actor_prim::{Actor, ActorContext, ActorRef},
    dispatcher::dispatch_executor::TokioExecutor,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    system::{ActorSystem, DispatcherConfig},
  },
};
use fraktor_utils_rs::core::sync::ArcShared;
use tokio::runtime::Handle;

struct Start;

struct GuardianActor {
  dispatcher: DispatcherConfig,
}

impl GuardianActor {
  fn new(dispatcher: DispatcherConfig) -> Self {
    Self { dispatcher }
  }

  fn child_props<F, A>(&self, factory: F) -> Props
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: Actor + Sync + 'static, {
    Props::from_fn(factory).with_dispatcher(self.dispatcher.clone())
  }
}

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let pong_props = self.child_props(|| PongActor);
      let pong = ctx.spawn_child(&pong_props).map_err(|_| ActorError::recoverable("failed to spawn pong"))?;

      let ping_props = self.child_props(|| PingActor);
      let ping = ctx.spawn_child(&ping_props).map_err(|_| ActorError::recoverable("failed to spawn ping"))?;

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
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(cmd) = message.downcast_ref::<StartPing>() {
      for index in 0..cmd.count {
        let payload = PingMessage { text: format!("ping-{}", index + 1), reply_to: cmd.reply_to.clone() };
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
      println!("[{:?}] received ping: {}", std::thread::current().id(), ping.text);
      let response = PongReply { text: ping.text.clone() };
      ping.reply_to.tell(AnyMessage::new(response)).map_err(|_| ActorError::recoverable("reply failed"))?;
    }
    Ok(())
  }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
  let handle = Handle::current();
  let dispatcher: DispatcherConfig = DispatcherConfig::from_executor(ArcShared::new(TokioExecutor::new(handle)));

  let props: Props = Props::from_fn({
    let dispatcher = dispatcher.clone();
    move || GuardianActor::new(dispatcher.clone())
  })
  .with_dispatcher(dispatcher.clone());

  let tick_driver = fraktor_actor_rs::std::scheduler::tick::StdTickDriverConfig::tokio_quickstart();
  let system = ActorSystem::new(&props, tick_driver).expect("system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  tokio::time::sleep(Duration::from_millis(50)).await;

  system.terminate().expect("terminate");

  termination.listener().await;
}
