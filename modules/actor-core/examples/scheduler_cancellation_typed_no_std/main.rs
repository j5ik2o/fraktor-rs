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
  label: String,
}

// Guardianアクターのコマンド
enum GuardianCommand {
  Start,
  Scheduled(ScheduledMessage),
}

struct GuardianActor {
  received_messages: u32,
}

impl GuardianActor {
  const fn new() -> Self {
    Self { received_messages: 0 }
  }
}

impl TypedActor<GuardianCommand> for GuardianActor {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, GuardianCommand>,
    message: &GuardianCommand,
  ) -> Result<(), ActorError> {
    match message {
      | GuardianCommand::Start => {
        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Guardian starting typed cancellation example...", std::thread::current().id());

        let target = ctx.self_ref();

        let scheduler_context = ctx.system().scheduler_context().expect("scheduler context");
        let scheduler_shared = scheduler_context.scheduler();
        let mut scheduler = scheduler_shared.lock();

        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Scheduling 3 typed messages...", std::thread::current().id());

        // 3つのメッセージをスケジュール
        let handle2 = scheduler.with(|typed_scheduler| {
          let msg1 = ScheduledMessage { label: String::from("Typed Message 1 (will execute)") };
          let cmd1 = GuardianCommand::Scheduled(msg1);
          typed_scheduler
            .schedule_once(Duration::from_millis(50), target.clone(), cmd1, None, None)
            .map_err(|_| ActorError::recoverable("failed to schedule 1"))?;

          let msg2 = ScheduledMessage { label: String::from("Typed Message 2 (will be cancelled)") };
          let cmd2 = GuardianCommand::Scheduled(msg2);
          let handle2 = typed_scheduler
            .schedule_once(Duration::from_millis(100), target.clone(), cmd2, None, None)
            .map_err(|_| ActorError::recoverable("failed to schedule 2"))?;

          let msg3 = ScheduledMessage { label: String::from("Typed Message 3 (will execute)") };
          let cmd3 = GuardianCommand::Scheduled(msg3);
          typed_scheduler
            .schedule_once(Duration::from_millis(150), target, cmd3, None, None)
            .map_err(|_| ActorError::recoverable("failed to schedule 3"))?;

          Ok(handle2)
        })?;

        // handle2をキャンセル
        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Cancelling typed message 2...", std::thread::current().id());

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
          "[{:?}] Typed scheduler ticks completed. Received {} messages (expected 2)",
          std::thread::current().id(),
          self.received_messages
        );
      },
      | GuardianCommand::Scheduled(msg) => {
        self.received_messages += 1;
        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Received: {}", std::thread::current().id(), msg.label);
      },
    }
    Ok(())
  }
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let props = TypedProps::new(GuardianActor::new);
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
