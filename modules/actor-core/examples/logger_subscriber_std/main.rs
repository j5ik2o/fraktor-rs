#![cfg(feature = "std")]

extern crate alloc;

use std::{thread, time::Duration};

use cellactor_actor_core_rs::{
  Actor, ActorContext, ActorError, ActorSystem, AnyMessage, AnyMessageView, LogLevel, LoggerSubscriber, LoggerWriter,
  Props,
};
use cellactor_utils_core_rs::sync::ArcShared;

struct Start;

struct StdoutLogger;

impl LoggerWriter for StdoutLogger {
  fn write(&self, event: &cellactor_actor_core_rs::LogEvent) {
    println!("[{:?}] {}", event.level(), event.message());
  }
}

struct Guardian;

impl Actor for Guardian {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    ctx.log(LogLevel::Info, "guardian pre_start");
    Ok(())
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      ctx.log(LogLevel::Info, "received Start message");
      ctx.stop_self().ok();
    }
    Ok(())
  }
}

fn main() {
  let props = Props::from_fn(|| Guardian);
  let system = ActorSystem::new(&props).expect("system");

  let writer: ArcShared<dyn LoggerWriter> = ArcShared::new(StdoutLogger);
  let subscriber = ArcShared::new(LoggerSubscriber::new(LogLevel::Info, writer));
  let _subscription = system.subscribe_event_stream(subscriber);

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  let termination = system.when_terminated();
  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::sleep(Duration::from_millis(10));
  }
}
