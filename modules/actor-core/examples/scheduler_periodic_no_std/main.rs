#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

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

// 周期的に送信されるメッセージ
struct PeriodicTick {
  count: u32,
}

struct Start;

struct GuardianActor {
  received_count: u32,
}

impl GuardianActor {
  const fn new() -> Self {
    Self { received_count: 0 }
  }
}

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Guardian starting periodic scheduler example...", std::thread::current().id());

      let target = ctx.self_ref();

      let scheduler_context = ctx.system().scheduler_context().expect("scheduler context");
      let scheduler_arc = scheduler_context.scheduler();
      let mut scheduler = scheduler_arc.lock();

      // 固定レート: 50msの初期遅延後、30msごとにメッセージを送信
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Scheduling at fixed rate: initial=50ms, interval=30ms", std::thread::current().id());

      let message = AnyMessage::new(PeriodicTick { count: 0 });
      let command = SchedulerCommand::SendMessage { receiver: target.clone(), message, dispatcher: None, sender: None };

      let _handle = scheduler
        .schedule_at_fixed_rate(Duration::from_millis(50), Duration::from_millis(30), command)
        .map_err(|_| ActorError::recoverable("failed to schedule"))?;

      // スケジューラを進める（デモ用）
      struct ManualOwner;
      let tick_handle = SchedulerTickHandle::scoped(&ManualOwner);
      let mut runner = SchedulerRunner::manual(&tick_handle);

      // 約200msのティックを進める（resolution=10msの場合、20ティック）
      // これにより、50ms後の最初の実行と、その後30ms間隔で数回実行される
      for _ in 0..30 {
        runner.inject_manual_ticks(1);
        runner.run_once(&mut scheduler);
      }

      #[cfg(not(target_os = "none"))]
      println!(
        "[{:?}] Scheduler ticks completed. Received {} periodic messages",
        std::thread::current().id(),
        self.received_count
      );
    } else if let Some(tick) = message.downcast_ref::<PeriodicTick>() {
      self.received_count += 1;
      #[cfg(not(target_os = "none"))]
      println!(
        "[{:?}] Received periodic tick #{} (payload count {})",
        std::thread::current().id(),
        self.received_count,
        tick.count,
      );
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
