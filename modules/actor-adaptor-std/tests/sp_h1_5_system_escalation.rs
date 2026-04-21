#![cfg(not(target_os = "none"))]

extern crate alloc;

use alloc::vec::Vec;
use core::time::Duration;

use fraktor_actor_adaptor_std_rs::std::actor::install_panic_invoke_guard;
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, ChildRef,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick_driver::TestTickDriver,
    setup::ActorSystemConfig,
    supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyConfig, SupervisorStrategyKind},
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

struct Start;
struct Crash;

struct PanicChild {
  log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl Actor for PanicChild {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("child_pre_start");
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("child_post_stop");
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Crash>().is_some() {
      panic!("panic child boom");
    }
    Ok(())
  }
}

struct PanicSupervisor {
  child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
  child_log:  ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl PanicSupervisor {
  fn new(
    child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
    child_log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
  ) -> Self {
    Self { child_slot, child_log }
  }
}

impl Actor for PanicSupervisor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() && self.child_slot.lock().is_none() {
      let child_log = self.child_log.clone();
      let child = ctx
        .spawn_child(&Props::from_fn(move || PanicChild { log: child_log.clone() }))
        .map_err(|_| ActorError::recoverable("spawn child"))?;
      self.child_slot.lock().replace(child);
    }
    Ok(())
  }

  fn supervisor_strategy(&self, _ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, Duration::from_secs(1), restart_on_escalate_only)
      .into()
  }
}

fn restart_on_escalate_only(error: &ActorError) -> SupervisorDirective {
  match error {
    | ActorError::Escalate(_) => SupervisorDirective::Restart,
    | ActorError::Recoverable(_) | ActorError::Fatal(_) => SupervisorDirective::Stop,
  }
}

#[test]
fn panic_guard_escalates_receive_panic_through_supervisor_path() {
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));
  let child_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let child_slot = child_slot.clone();
    let child_log = child_log.clone();
    move || PanicSupervisor::new(child_slot.clone(), child_log.clone())
  });

  let config = install_panic_invoke_guard(ActorSystemConfig::new(TestTickDriver::default()));
  let system = ActorSystem::create_with_config(&props, config).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(Start));
  let mut child = child_slot.lock().clone().expect("child");
  child.tell(AnyMessage::new(Crash));

  assert_eq!(child_log.lock().as_slice(), ["child_pre_start", "child_post_stop", "child_pre_start"]);
}
