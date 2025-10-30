#![cfg(feature = "std")]

extern crate alloc;

use cellactor_actor_core_rs::{Actor, ActorContext, ActorError, ActorSystem, AnyMessageView, Props};

struct IdleGuardian;

impl Actor for IdleGuardian {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn terminate_signals_future() {
  let system = ActorSystem::new(Props::from_fn(|| IdleGuardian)).expect("system");

  let termination = system.when_terminated();
  assert!(!termination.is_ready(), "system should be running before terminate");

  system.terminate().expect("terminate");
  system.run_until_terminated();

  assert!(termination.is_ready(), "termination future must be completed after shutdown");
}

#[test]
fn when_terminated_is_ready_after_shutdown() {
  let system = ActorSystem::new(Props::from_fn(|| IdleGuardian)).expect("system");
  system.terminate().expect("terminate");
  system.run_until_terminated();

  let termination = system.when_terminated();
  assert!(termination.is_ready(), "termination future must remain resolved");

  // Idempotency check.
  system.terminate().expect("terminate again");
}
