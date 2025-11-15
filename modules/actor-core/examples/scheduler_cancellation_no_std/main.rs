#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::string::String;
use core::time::Duration;

use fraktor_actor_core_rs::{
  actor_prim::{Actor, ActorContext},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  scheduler::{SchedulerCommand, SchedulerRunner},
  system::ActorSystem,
};
use fraktor_utils_core_rs::time::SchedulerTickHandle;

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
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
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

      // スケジューラを進める（デモ用）
      struct ManualOwner;
      let tick_handle = SchedulerTickHandle::scoped(&ManualOwner);
      let mut runner = SchedulerRunner::manual(&tick_handle);

      for _ in 0..20 {
        runner.inject_manual_ticks(1);
        runner.run_once(&mut scheduler);
      }

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
  use std::thread;

  let props = Props::from_fn(GuardianActor::new);
  let system = ActorSystem::new(&props).expect("system");
  let termination = system.when_terminated();
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  // スケジューラが動作する時間を与える
  thread::sleep(std::time::Duration::from_millis(300));

  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
