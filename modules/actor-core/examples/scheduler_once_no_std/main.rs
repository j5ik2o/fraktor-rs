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

// アクターに送信されるスケジュール済みメッセージ
struct ScheduledMessage {
  text: String,
}

struct Start;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Guardian starting scheduler example...", std::thread::current().id());

      // スケジューラを取得（システムから）
      // 注: 実際のシステムではスケジューラはシステムによって管理されるため、
      // この例ではスケジューラの使用方法を示すためのものです
      let target = ctx.self_ref();

      // 100msの遅延後にメッセージを送信するようスケジュール
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Scheduling message with 100ms delay...", std::thread::current().id());

      let scheduler_context = ctx.system().scheduler_context().expect("scheduler context");
      let scheduler_arc = scheduler_context.scheduler();
      let mut scheduler = scheduler_arc.lock();

      let message = AnyMessage::new(ScheduledMessage { text: String::from("Hello from scheduler!") });
      let command = SchedulerCommand::SendMessage { receiver: target.clone(), message, dispatcher: None, sender: None };

      let _handle = scheduler
        .schedule_once(Duration::from_millis(100), command)
        .map_err(|_| ActorError::recoverable("failed to schedule"))?;

      // スケジューラを進める（デモ用）
      struct ManualOwner;
      let tick_handle = SchedulerTickHandle::scoped(&ManualOwner);
      let mut runner = SchedulerRunner::manual(&tick_handle);

      // 十分なティック数を進める（100ms / resolution）
      // デフォルトのresolutionは10msなので、100ms = 10ティック
      for _ in 0..15 {
        runner.inject_manual_ticks(1);
        runner.run_once(&mut scheduler);
      }

      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Scheduler ticks completed", std::thread::current().id());
    } else if let Some(msg) = message.downcast_ref::<ScheduledMessage>() {
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Received scheduled message: {}", std::thread::current().id(), msg.text);
    }
    Ok(())
  }
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let props = Props::from_fn(|| GuardianActor);
  let system = ActorSystem::new(&props).expect("system");
  let termination = system.when_terminated();
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  // スケジューラが動作する時間を与える
  thread::sleep(std::time::Duration::from_millis(200));

  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
