#![cfg_attr(all(not(test), target_os = "none"), no_std)]

#[cfg(any(test, feature = "test-support"))]
mod demo {
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

  pub struct ScheduledMessage {
    text: String,
  }

  pub struct Start;

  struct GuardianActor;

  impl Actor for GuardianActor {
    fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
      if message.downcast_ref::<Start>().is_some() {
        let target = ctx.self_ref();
        let scheduler_context = ctx.system().scheduler_context().expect("scheduler context");
        let scheduler_arc = scheduler_context.scheduler();
        let mut scheduler = scheduler_arc.lock();
        let mut subscription = scheduler.subscribe_diagnostics(100);

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

        struct ManualOwner;
        let tick_handle = SchedulerTickHandle::scoped(&ManualOwner);
        let mut runner = SchedulerRunner::manual(&tick_handle);
        for _ in 0..20 {
          runner.inject_manual_ticks(1);
          runner.run_once(&mut scheduler);
        }

        let _ = subscription.drain();
      } else if message.downcast_ref::<ScheduledMessage>().is_some() {
        #[cfg(not(target_os = "none"))]
        println!("received diagnostics payload");
      }
      Ok(())
    }
  }

  pub fn run() {
    let props = Props::from_fn(|| GuardianActor);
    let system = ActorSystem::new(&props).expect("system");
    let termination = system.when_terminated();
    system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
    #[cfg(not(target_os = "none"))]
    std::thread::sleep(std::time::Duration::from_millis(300));
    system.terminate().expect("terminate");
    while !termination.is_ready() {}
  }
}

#[cfg(any(test, feature = "test-support"))]
fn main() {
  demo::run();
}

#[cfg(not(any(test, feature = "test-support")))]
fn main() {}
