#![cfg(feature = "std")]

use core::num::NonZeroUsize;
use std::{thread, time::Duration};

use cellactor_actor_core_rs::{
  Actor, ActorContext, ActorError, ActorRef, AnyMessage, AnyMessageView, ChildRef, EventStreamEvent,
  EventStreamSubscriber, LogEvent, LogLevel, LoggerSubscriber, LoggerWriter, MailboxConfig, MailboxOverflowStrategy,
  MailboxPolicy, Props,
};
use cellactor_actor_std_rs::{StdActorSystem, StdToolbox};
use cellactor_utils_core_rs::sync::ArcShared;

struct Start;
struct LogDeadletters;

struct StdoutLogger;

impl LoggerWriter for StdoutLogger {
  fn write(&self, event: &LogEvent) {
    println!("[LOG {:?}] origin={:?} message={}", event.level(), event.origin(), event.message());
  }
}

struct DeadletterPrinter;

impl EventStreamSubscriber<StdToolbox> for DeadletterPrinter {
  fn on_event(&self, event: &EventStreamEvent<StdToolbox>) {
    if let EventStreamEvent::Deadletter(entry) = event {
      println!(
        "[DEADLETTER] reason={:?} recipient={:?} message_type={:?}",
        entry.reason(),
        entry.recipient(),
        entry.message().payload().type_id()
      );
    }
  }
}

struct GuardianActor;

impl Actor<StdToolbox> for GuardianActor {
  fn receive(
    &mut self,
    ctx: &mut ActorContext<'_, StdToolbox>,
    message: AnyMessageView<'_, StdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let mailbox_policy =
        MailboxPolicy::bounded(NonZeroUsize::new(1).expect("non-zero"), MailboxOverflowStrategy::DropNewest, None);
      let props = Props::from_fn(|| EchoActor).with_mailbox(MailboxConfig::new(mailbox_policy));
      let child = ctx.spawn_child(&props).map_err(|error| ActorError::fatal(format!("spawn failed: {:?}", error)))?;
      let actor_ref = child.actor_ref();

      send_or_log(ctx, &actor_ref, AnyMessage::new("first"));
      send_or_log(ctx, &actor_ref, AnyMessage::new("second"));
      suspend_or_log(ctx, &child);
      send_or_log(ctx, &actor_ref, AnyMessage::new("third"));
      send_or_log(ctx, &actor_ref, AnyMessage::new(LogDeadletters));
      ctx.stop_self().ok();
    }
    Ok(())
  }
}

struct EchoActor;

impl Actor<StdToolbox> for EchoActor {
  fn receive(
    &mut self,
    ctx: &mut ActorContext<'_, StdToolbox>,
    message: AnyMessageView<'_, StdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<LogDeadletters>().is_some() {
      let entries = ctx.system().deadletters();
      println!("[DEADLETTER SNAPSHOT] {} entries", entries.len());
      for entry in entries {
        println!("  - reason={:?}, recipient={:?}", entry.reason(), entry.recipient());
      }
    }
    Ok(())
  }
}

fn main() {
  let props: Props<StdToolbox> = Props::from_fn(|| GuardianActor);
  let system = StdActorSystem::new(&props).expect("actor system を初期化できること");

  let logger_writer: ArcShared<dyn LoggerWriter> = ArcShared::new(StdoutLogger);
  let logger: ArcShared<dyn EventStreamSubscriber<StdToolbox>> =
    ArcShared::new(LoggerSubscriber::new(LogLevel::Info, logger_writer));
  let _log_subscription = system.subscribe_event_stream(&logger);

  let printer: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = ArcShared::new(DeadletterPrinter);
  let _deadletter_subscription = system.subscribe_event_stream(&printer);

  let guardian: ActorRef<StdToolbox> = system.user_guardian_ref();
  guardian.tell(AnyMessage::new(Start)).expect("ガーディアンへ Start を送信できること");

  thread::sleep(Duration::from_millis(50));

  system.terminate().expect("システム停止要求が成功すること");
  let termination = system.when_terminated();
  while !termination.is_ready() {
    thread::sleep(Duration::from_millis(10));
  }
}

fn send_or_log(ctx: &ActorContext<'_, StdToolbox>, target: &ActorRef<StdToolbox>, message: AnyMessage<StdToolbox>) {
  if let Err(error) = target.tell(message) {
    ctx.log(LogLevel::Warn, format!("send failed: {:?}", error));
  }
}

fn suspend_or_log(ctx: &ActorContext<'_, StdToolbox>, child: &ChildRef<StdToolbox>) {
  if let Err(error) = ctx.suspend_child(child) {
    ctx.log(LogLevel::Warn, format!("suspend failed: {:?}", error));
  }
}
