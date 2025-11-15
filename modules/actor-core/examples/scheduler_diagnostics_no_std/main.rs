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

struct ScheduledMessage {
  text: String,
}

struct Start;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Guardian starting diagnostics example...", std::thread::current().id());

      let target = ctx.self_ref();

      let scheduler_context = ctx.system().scheduler_context().expect("scheduler context");
      let scheduler_arc = scheduler_context.scheduler();
      let mut scheduler = scheduler_arc.lock();

      // 診断ストリームをサブスクライブ
      let mut subscription = scheduler.subscribe_diagnostics(100);

      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Subscribed to diagnostics stream", std::thread::current().id());

      // いくつかのメッセージをスケジュール
      for i in 0..3 {
        let msg = AnyMessage::new(ScheduledMessage { text: alloc::format!("Message {}", i + 1) });
        let command = SchedulerCommand::SendMessage {
          receiver:   target.clone(),
          message:    msg,
          dispatcher: None,
          sender:     None,
        };

        let _handle = scheduler
          .schedule_once(Duration::from_millis(50 * (i + 1)), command)
          .map_err(|_| ActorError::recoverable("failed to schedule"))?;
      }

      // スケジューラを進める
      struct ManualOwner;
      let tick_handle = SchedulerTickHandle::scoped(&ManualOwner);
      let mut runner = SchedulerRunner::manual(&tick_handle);

      for _ in 0..20 {
        runner.inject_manual_ticks(1);
        runner.run_once(&mut scheduler);
      }

      // 診断イベントを取得
      let events = subscription.drain();

      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Diagnostics events collected: {} events", std::thread::current().id(), events.len());

      // スケジューラダンプを取得
      let dump = scheduler.dump();

      #[cfg(not(target_os = "none"))]
      {
        println!("[{:?}] Scheduler dump:", std::thread::current().id());
        println!("  Current tick: {}", dump.current_tick());
        println!("  Resolution: {:?}", dump.resolution());
        println!("  Active jobs: {}", dump.jobs().len());
        println!("  Metrics - active timers: {}", dump.metrics().active_timers());
        println!("  Metrics - dropped total: {}", dump.metrics().dropped_total());
      }
    } else if let Some(msg) = message.downcast_ref::<ScheduledMessage>() {
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Received: {}", std::thread::current().id(), msg.text);
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
  thread::sleep(std::time::Duration::from_millis(300));

  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
