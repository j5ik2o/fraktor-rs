#![cfg(feature = "std")]

extern crate alloc;

use alloc::format;
use std::{thread, time::Duration};

use cellactor_actor_core_rs::{
  Actor, ActorContext, ActorError, ActorRef, ActorSystem, AnyMessage, AnyMessageView, EventStreamEvent,
  EventStreamSubscriber, LogEvent, LogLevel, LoggerSubscriber, LoggerWriter, Props, SupervisorOptions,
  SupervisorStrategy, SupervisorStrategyKind,
};
use cellactor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

struct Start;
struct Trigger;

struct StdoutLogger;

impl LoggerWriter for StdoutLogger {
  fn write(&self, event: &LogEvent) {
    println!("[{:?}] {}", event.level(), event.message());
  }
}

struct LifecyclePrinter;

impl EventStreamSubscriber for LifecyclePrinter {
  fn on_event(&self, event: &EventStreamEvent) {
    match event {
      | EventStreamEvent::Lifecycle(lifecycle) => {
        println!("[LIFECYCLE] pid={:?} stage={:?}", lifecycle.pid(), lifecycle.stage())
      },
      | EventStreamEvent::Deadletter(entry) => {
        println!("[DEADLETTER] reason={:?} message_type={:?}", entry.reason(), entry.message().payload().type_id())
      },
      | EventStreamEvent::Log(_) | EventStreamEvent::Mailbox(_) => {},
    }
  }
}

struct Guardian {
  child: SpinSyncMutex<Option<ActorRef>>,
}

impl Guardian {
  fn new() -> Self {
    Self { child: SpinSyncMutex::new(None) }
  }
}

impl Actor for Guardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      ctx.log(LogLevel::Info, "guardian starting child");
      let strategy =
        SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 2, Duration::from_secs(1), |error| match error {
          | ActorError::Recoverable(_) => cellactor_actor_core_rs::SupervisorDirective::Restart,
          | ActorError::Fatal(_) => cellactor_actor_core_rs::SupervisorDirective::Stop,
        });
      let props = Props::from_fn(FussyWorker::new).with_supervisor(SupervisorOptions::new(strategy));
      let child =
        ctx.spawn_child(&props).map_err(|err| ActorError::fatal(format!("failed to spawn child: {:?}", err)))?;
      self.child.lock().replace(child.actor_ref().clone());

      for _ in 0..4 {
        ctx.log(LogLevel::Info, "sending trigger to child");
        child.actor_ref().tell(AnyMessage::new(Trigger)).expect("tell");
        thread::sleep(Duration::from_millis(50));
      }

      ctx.stop_self().ok();
    }
    Ok(())
  }
}

struct FussyWorker {
  crashes_remaining: i32,
}

impl FussyWorker {
  fn new() -> Self {
    Self { crashes_remaining: 2 }
  }
}

impl Actor for FussyWorker {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    ctx.log(LogLevel::Info, "worker pre_start");
    Ok(())
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if self.crashes_remaining >= 0 {
      ctx.log(LogLevel::Warn, format!("simulated crash (remaining {})", self.crashes_remaining));
      self.crashes_remaining -= 1;
      Err(ActorError::recoverable("simulated failure"))
    } else {
      ctx.log(LogLevel::Info, "work succeeded");
      Ok(())
    }
  }

  fn post_stop(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    ctx.log(LogLevel::Info, "worker stopping");
    Ok(())
  }
}

fn main() {
  let props = Props::from_fn(Guardian::new);
  let system = ActorSystem::new(&props).expect("system");

  let logger_writer: ArcShared<dyn LoggerWriter> = ArcShared::new(StdoutLogger);
  let logger: ArcShared<dyn EventStreamSubscriber> =
    ArcShared::new(LoggerSubscriber::new(LogLevel::Info, logger_writer));
  let _logger_subscription = system.subscribe_event_stream(&logger);

  let lifecycle: ArcShared<dyn EventStreamSubscriber> = ArcShared::new(LifecyclePrinter);
  let _lifecycle_subscription = system.subscribe_event_stream(&lifecycle);

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  let termination = system.when_terminated();
  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::sleep(Duration::from_millis(20));
  }
}
