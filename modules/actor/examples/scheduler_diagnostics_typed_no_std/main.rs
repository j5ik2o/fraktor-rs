//! Scheduler diagnostics example with typed actors for no_std environments.
//!
//! Demonstrates `SchedulerDiagnosticsSubscription` combined with the
//! `TypedActor` API to observe scheduler internal state.

#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::string::String;
use core::time::Duration;
#[cfg(not(target_os = "none"))]
use std::{thread, time::Duration as StdDuration};

use fraktor_actor_rs::core::{
  error::ActorError,
  scheduler::SchedulerDiagnosticsSubscription,
  typed::{
    TypedActorSystem, TypedProps,
    actor::{TypedActor, TypedActorContext},
  },
};

#[cfg(not(target_os = "none"))]
#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;
#[cfg(not(target_os = "none"))]
#[derive(Clone)]
struct ScheduledMessage {
  text: String,
}

enum GuardianCommand {
  Start,
  Scheduled(ScheduledMessage),
  Dump,
}

struct GuardianActor {
  diagnostics: Option<SchedulerDiagnosticsSubscription>,
  received:    u32,
}

impl GuardianActor {
  const fn new() -> Self {
    Self { diagnostics: None, received: 0 }
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
        println!("[{:?}] Guardian starting typed diagnostics example...", std::thread::current().id());

        let scheduler_shared = ctx.system().scheduler();
        let target = ctx.self_ref();

        self.diagnostics = Some(scheduler_shared.with_write(|s| s.subscribe_diagnostics(128)));

        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Subscribed to typed diagnostics stream", std::thread::current().id());

        scheduler_shared.with_write(|typed_scheduler| {
          for i in 0..3 {
            let msg = ScheduledMessage { text: alloc::format!("Typed Message {}", i + 1) };
            let cmd = GuardianCommand::Scheduled(msg);
            typed_scheduler
              .schedule_once(Duration::from_millis(50 * (i + 1)), target.clone(), cmd, None, None)
              .map_err(|_| ActorError::recoverable("failed to schedule"))?;
          }
          typed_scheduler
            .schedule_once(Duration::from_millis(250), target, GuardianCommand::Dump, None, None)
            .map_err(|_| ActorError::recoverable("failed to schedule diagnostics dump"))?;
          Ok(())
        })?;
      },
      | GuardianCommand::Scheduled(msg) => {
        self.received += 1;
        #[cfg(not(target_os = "none"))]
        println!("[{:?}] Received: {}", std::thread::current().id(), msg.text);
      },
      | GuardianCommand::Dump => {
        if let Some(subscription) = self.diagnostics.as_mut() {
          let events = subscription.drain();
          #[cfg(not(target_os = "none"))]
          println!(
            "[{:?}] drained {} diagnostics events ({} scheduled messages processed)",
            std::thread::current().id(),
            events.len(),
            self.received,
          );
        }
      },
    }
    Ok(())
  }
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::process;

  let props = TypedProps::new(GuardianActor::new);
  let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");
  system.user_guardian_ref().tell(GuardianCommand::Start).expect("start");
  thread::sleep(StdDuration::from_millis(400));
  process::exit(0);
}

#[cfg(target_os = "none")]
fn main() {}
