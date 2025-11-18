#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::string::String;
use core::time::Duration;

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContext},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  scheduler::SchedulerCommand,
  system::ActorSystem,
};

#[cfg(not(target_os = "none"))]
#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;
#[cfg(not(target_os = "none"))]
// スケジュールされたメッセージ
struct ScheduledMessage {
  label: String,
}

struct Start;

struct GuardianActor {
  received_messages: u32,
}

impl GuardianActor {
  const fn new() -> Self {
    Self { received_messages: 0 }
  }
}

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageViewGeneric<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Guardian starting cancellation example...", std::thread::current().id());

      let target = ctx.self_ref();

      let scheduler_context = ctx.system().scheduler_context().expect("scheduler context");
      let scheduler_arc = scheduler_context.scheduler();
      let mut scheduler = scheduler_arc.lock();

      // 3つのメッセージをスケジュール
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Scheduling 3 messages...", std::thread::current().id());

      let msg1 = AnyMessage::new(ScheduledMessage { label: String::from("Message 1 (will execute)") });
      let command1 = SchedulerCommand::SendMessage {
        receiver:   target.clone(),
        message:    msg1,
        dispatcher: None,
        sender:     None,
      };
      let _handle1 = scheduler
        .schedule_once(Duration::from_millis(50), command1)
        .map_err(|_| ActorError::recoverable("failed to schedule 1"))?;

      let msg2 = AnyMessage::new(ScheduledMessage { label: String::from("Message 2 (will be cancelled)") });
      let command2 = SchedulerCommand::SendMessage {
        receiver:   target.clone(),
        message:    msg2,
        dispatcher: None,
        sender:     None,
      };
      let handle2 = scheduler
        .schedule_once(Duration::from_millis(100), command2)
        .map_err(|_| ActorError::recoverable("failed to schedule 2"))?;

      let msg3 = AnyMessage::new(ScheduledMessage { label: String::from("Message 3 (will execute)") });
      let command3 = SchedulerCommand::SendMessage {
        receiver:   target.clone(),
        message:    msg3,
        dispatcher: None,
        sender:     None,
      };
      let _handle3 = scheduler
        .schedule_once(Duration::from_millis(150), command3)
        .map_err(|_| ActorError::recoverable("failed to schedule 3"))?;

      // handle2をキャンセル
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Cancelling message 2...", std::thread::current().id());

      let cancelled = scheduler.cancel(&handle2);

      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Cancellation result: {}", std::thread::current().id(), cancelled);

      #[cfg(not(target_os = "none"))]
      println!(
        "[{:?}] Scheduler ticks completed. Received {} messages (expected 2)",
        std::thread::current().id(),
        self.received_messages
      );
    } else if let Some(msg) = message.downcast_ref::<ScheduledMessage>() {
      self.received_messages += 1;
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Received: {}", std::thread::current().id(), msg.label);
    }
    Ok(())
  }
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::{process, thread};

  let props = Props::from_fn(GuardianActor::new);
  let bootstrap = ActorSystem::new(&props, no_std_tick_driver_support::hardware_tick_driver_config()).expect("system");
  bootstrap.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  // スケジューラが動作する時間を与える
  thread::sleep(std::time::Duration::from_millis(300));

  process::exit(0);
}

#[cfg(target_os = "none")]
fn main() {}
