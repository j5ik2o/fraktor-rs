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

// スケジュール済みメッセージの型
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
        println!("[{:?}] Guardian starting typed scheduler example...", std::thread::current().id());

        let target = ctx.self_ref();

        let scheduler_context = ctx.system().scheduler_context().expect("scheduler context");
        let scheduler_shared = scheduler_context.scheduler();
        let mut scheduler = scheduler_shared.lock();

        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Scheduling typed message with 100ms delay...", std::thread::current().id());

        let message = ScheduledMessage { text: String::from("Hello from typed scheduler!") };
        let command = GuardianCommand::Scheduled(message);

        // TypedScheduler::schedule_onceを使用
        let _handle = scheduler.with(|typed_scheduler| {
          typed_scheduler
            .schedule_once(Duration::from_millis(100), target, command, None, None)
            .map_err(|_| ActorError::recoverable("failed to schedule"))
        })?;

        // スケジューラを進める（デモ用）
        struct ManualOwner;
        let tick_handle = SchedulerTickHandle::scoped(&ManualOwner);
        let mut runner = SchedulerRunner::manual(&tick_handle);

        for _ in 0..15 {
          runner.inject_manual_ticks(1);
          runner.run_once(&mut scheduler);
        }

        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Typed scheduler ticks completed", std::thread::current().id());
      },
      | GuardianCommand::Scheduled(msg) => {
        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Received scheduled typed message: {}", std::thread::current().id(), msg.text);
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
  thread::sleep(std::time::Duration::from_millis(200));

  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
