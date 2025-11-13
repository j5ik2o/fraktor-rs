#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::string::String;
use core::time::Duration;

use fraktor_actor_core_rs::{
  error::ActorError,
  scheduler::SchedulerRunner,
  typed::{
    TypedActorSystem, TypedProps,
    actor_prim::{TypedActor, TypedActorContext},
  },
};
use fraktor_utils_core_rs::time::SchedulerTickHandle;

// スケジュールされたメッセージ
#[derive(Clone)]
struct ScheduledMessage {
  text: String,
}

// Guardianアクターのコマンド
enum GuardianCommand {
  Start,
  Scheduled(ScheduledMessage),
}

struct GuardianActor;

impl TypedActor<GuardianCommand> for GuardianActor {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, GuardianCommand>,
    message: &GuardianCommand,
  ) -> Result<(), ActorError> {
    match message {
      | GuardianCommand::Start => {
        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Guardian starting typed diagnostics example...", std::thread::current().id());

        let target = ctx.self_ref();

        let scheduler_context = ctx.system().scheduler_context().expect("scheduler context");
        let scheduler_shared = scheduler_context.scheduler();
        let mut scheduler = scheduler_shared.lock();

        // 診断ストリームをサブスクライブ
        let mut subscription = scheduler.subscribe_diagnostics(100);

        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Subscribed to typed diagnostics stream", std::thread::current().id());

        // いくつかのメッセージをスケジュール
        scheduler.with(|typed_scheduler| {
          for i in 0..3 {
            let msg = ScheduledMessage { text: alloc::format!("Typed Message {}", i + 1) };
            let cmd = GuardianCommand::Scheduled(msg);

            typed_scheduler
              .schedule_once(Duration::from_millis(50 * (i + 1)), target.clone(), cmd, None, None)
              .map_err(|_| ActorError::recoverable("failed to schedule"))?;
          }
          Ok(())
        })?;

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
        println!("[{:?}] Typed diagnostics events collected: {} events", std::thread::current().id(), events.len());

        // スケジューラダンプを取得
        let dump = scheduler.dump();

        #[cfg(not(target_os = "none"))]
        {
          println!("[{:?}] Typed scheduler dump:", std::thread::current().id());
          println!("  Current tick: {}", dump.current_tick());
          println!("  Resolution: {:?}", dump.resolution());
          println!("  Active jobs: {}", dump.jobs().len());
          println!("  Metrics - active timers: {}", dump.metrics().active_timers());
          println!("  Metrics - dropped total: {}", dump.metrics().dropped_total());
        }
      },
      | GuardianCommand::Scheduled(msg) => {
        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Received: {}", std::thread::current().id(), msg.text);
      },
    }
    Ok(())
  }
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let props = TypedProps::new(|| GuardianActor);
  let system = TypedActorSystem::new(&props).expect("system");
  let termination = system.as_untyped().when_terminated();
  system.user_guardian_ref().tell(GuardianCommand::Start).expect("start");

  // スケジューラが動作する時間を与える
  thread::sleep(std::time::Duration::from_millis(300));

  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
