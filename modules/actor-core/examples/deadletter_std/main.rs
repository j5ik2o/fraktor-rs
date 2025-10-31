#![cfg(feature = "std")]

extern crate alloc;

use alloc::format;
use core::num::NonZeroUsize;
use std::{thread, time::Duration};

use cellactor_actor_core_rs::{
  Actor, ActorContext, ActorError, ActorRef, ActorSystem, AnyMessage, AnyMessageView, EventStreamEvent,
  EventStreamSubscriber, LogLevel, LoggerSubscriber, LoggerWriter, MailboxConfig, MailboxOverflowStrategy,
  MailboxPolicy, Props,
};
use cellactor_utils_core_rs::sync::ArcShared;

struct Start;
struct LogDeadletters;

struct StdoutLogger;

impl LoggerWriter for StdoutLogger {
  fn write(&self, event: &cellactor_actor_core_rs::LogEvent) {
    println!("[LOG {:?}] {}", event.level(), event.message());
  }
}

struct DeadletterPrinter;

impl EventStreamSubscriber for DeadletterPrinter {
  fn on_event(&self, event: &EventStreamEvent) {
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

struct Guardian;

impl Actor for Guardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      // Spawn a child with zero-capacity mailbox to trigger deadletters.
      let mailbox_policy =
        MailboxPolicy::bounded(NonZeroUsize::new(1).unwrap(), MailboxOverflowStrategy::DropNewest, None);
      let props = Props::from_fn(|| Echo).with_mailbox(MailboxConfig::new(mailbox_policy));
      let child = ctx.spawn_child(&props).map_err(|err| ActorError::fatal(format!("spawn failed: {:?}", err)))?;
      let actor_ref = child.actor_ref();

      // Fill the mailbox, then suspend to force further messages into deadletter.
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

struct Echo;

impl Actor for Echo {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
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
  let props = Props::from_fn(|| Guardian);
  let system = ActorSystem::new(&props).expect("system");

  let logger_writer: ArcShared<dyn LoggerWriter> = ArcShared::new(StdoutLogger);
  let logger = ArcShared::new(LoggerSubscriber::new(LogLevel::Info, logger_writer));
  let _log_subscription = system.subscribe_event_stream(logger);

  let printer = ArcShared::new(DeadletterPrinter);
  let _deadletter_subscription = system.subscribe_event_stream(printer);

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  let termination = system.when_terminated();
  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::sleep(Duration::from_millis(20));
  }
}

fn send_or_log(ctx: &ActorContext<'_>, target: &ActorRef, message: AnyMessage) {
  if let Err(error) = target.tell(message) {
    ctx.log(LogLevel::Warn, format!("send failed: {:?}", error));
  }
}

fn suspend_or_log(ctx: &ActorContext<'_>, child: &cellactor_actor_core_rs::ChildRef) {
  if let Err(error) = ctx.suspend_child(child) {
    ctx.log(LogLevel::Warn, format!("suspend failed: {:?}", error));
  }
}
