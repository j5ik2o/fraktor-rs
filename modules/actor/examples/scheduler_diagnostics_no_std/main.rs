#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::string::String;
use core::time::Duration;
#[cfg(not(target_os = "none"))]
use std::{thread, time::Duration as StdDuration};

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContext},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  scheduler::{SchedulerCommand, SchedulerDiagnosticsSubscription},
  system::ActorSystem,
};

#[cfg(not(target_os = "none"))]
#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;
#[cfg(not(target_os = "none"))]
struct ScheduledMessage {
  text: String,
}

struct DumpDiagnostics;

struct Start;

struct GuardianActor {
  diagnostics: Option<SchedulerDiagnosticsSubscription>,
  received:    u32,
}

impl GuardianActor {
  const fn new() -> Self {
    Self { diagnostics: None, received: 0 }
  }
}

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageViewGeneric<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] subscribing scheduler diagnostics", std::thread::current().id());

      let scheduler_context = ctx.system().scheduler_context().expect("scheduler context");
      let scheduler_arc = scheduler_context.scheduler();
      let mut scheduler = scheduler_arc.lock();
      self.diagnostics = Some(scheduler.subscribe_diagnostics(128));
      let target = ctx.self_ref();

      for i in 0..3 {
        let msg = AnyMessage::new(ScheduledMessage { text: alloc::format!("Message {}", i + 1) });
        let command = SchedulerCommand::SendMessage {
          receiver:   target.clone(),
          message:    msg,
          dispatcher: None,
          sender:     None,
        };
        scheduler
          .schedule_once(Duration::from_millis(50 * (i + 1)), command)
          .map_err(|_| ActorError::recoverable("failed to schedule"))?;
      }

      let dump = SchedulerCommand::SendMessage {
        receiver:   target,
        message:    AnyMessage::new(DumpDiagnostics),
        dispatcher: None,
        sender:     None,
      };
      scheduler
        .schedule_once(Duration::from_millis(250), dump)
        .map_err(|_| ActorError::recoverable("failed to schedule diagnostics dump"))?;
    } else if let Some(msg) = message.downcast_ref::<ScheduledMessage>() {
      self.received += 1;
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] diagnostics payload received: {}", std::thread::current().id(), msg.text);
    } else if message.downcast_ref::<DumpDiagnostics>().is_some() {
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
    }
    Ok(())
  }
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::process;

  let props = Props::from_fn(GuardianActor::new);
  let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
  let bootstrap = ActorSystem::new(&props, tick_driver).expect("system");
  bootstrap.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
  thread::sleep(StdDuration::from_millis(400));
  process::exit(0);
}

#[cfg(target_os = "none")]
fn main() {}
