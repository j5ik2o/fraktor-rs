#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use core::time::Duration;

use fraktor_actor_rs::core::{
  error::ActorError,
  typed::{
    TypedActorSystemBuilder, TypedProps,
    actor_prim::{TypedActor, TypedActorContext},
  },
};

#[cfg(not(target_os = "none"))]
#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;
#[cfg(not(target_os = "none"))]
// 周期的に送信されるメッセージ
#[derive(Clone)]
struct PeriodicTick {
  sequence: u32,
}

// Guardianアクターのコマンド
enum GuardianCommand {
  Start,
  Tick(PeriodicTick),
}

struct GuardianActor {
  received_count: u32,
}

impl GuardianActor {
  const fn new() -> Self {
    Self { received_count: 0 }
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
        println!("[{:?}] Guardian starting typed periodic scheduler example...", std::thread::current().id());

        let target = ctx.self_ref();

        let scheduler_context = ctx.system().scheduler_context().expect("scheduler context");
        let scheduler_shared = scheduler_context.scheduler();
        let mut scheduler = scheduler_shared.lock();

        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Scheduling at fixed rate: initial=50ms, interval=30ms", std::thread::current().id());

        let message = PeriodicTick { sequence: 0 };
        let command = GuardianCommand::Tick(message);

        // TypedScheduler::schedule_at_fixed_rateを使用
        scheduler.with(|typed_scheduler| {
          typed_scheduler
            .schedule_at_fixed_rate(Duration::from_millis(50), Duration::from_millis(30), target, command, None, None)
            .map_err(|_| ActorError::recoverable("failed to schedule"))
        })?;

        #[cfg(not(target_os = "none"))]
        println!(
          "[{:?}] Typed scheduler ticks completed. Received {} periodic messages",
          std::thread::current().id(),
          self.received_count
        );
      },
      | GuardianCommand::Tick(PeriodicTick { sequence }) => {
        self.received_count += 1;
        #[cfg(not(target_os = "none"))]
        println!(
          "[{:?}] Received typed periodic tick #{} (payload seq {})",
          std::thread::current().id(),
          self.received_count,
          sequence,
        );
      },
    }
    Ok(())
  }
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::{process, thread};

  let props = TypedProps::new(GuardianActor::new);
  let system = TypedActorSystemBuilder::new(props)
    .with_tick_driver(no_std_tick_driver_support::hardware_tick_driver_config())
    .build()
    .expect("system");
  system.user_guardian_ref().tell(GuardianCommand::Start).expect("start");

  // スケジューラが動作する時間を与える
  thread::sleep(std::time::Duration::from_millis(300));

  process::exit(0);
}

#[cfg(target_os = "none")]
fn main() {}
