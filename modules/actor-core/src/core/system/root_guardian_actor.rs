//! Internal root guardian that supervises `/user` and `/system`.

use fraktor_utils_core_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  actor_prim::{Actor, ActorContextGeneric, Pid, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
  system::SystemStateGeneric,
};

/// Root guardian actor responsible for watching the system guardian.
pub(crate) struct RootGuardianActor;

impl RootGuardianActor {
  /// Creates a new root guardian instance.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self
  }

  fn watch_system_guardian<TB: RuntimeToolbox + 'static>(
    ctx: &mut ActorContextGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(cell) = ctx.system().state().system_guardian() {
      let system_ref: ActorRefGeneric<TB> = cell.actor_ref();
      ctx.watch(&system_ref).map_err(|error| ActorError::from_send_error(&error))
    } else {
      Ok(())
    }
  }

  fn handle_system_terminated<TB: RuntimeToolbox + 'static>(state: &ArcShared<SystemStateGeneric<TB>>) {
    state.mark_terminated();
  }
}

impl<TB: RuntimeToolbox + 'static> Actor<TB> for RootGuardianActor {
  fn pre_start(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    Self::watch_system_guardian(ctx)
  }

  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  fn on_terminated(&mut self, ctx: &mut ActorContextGeneric<'_, TB>, _terminated: Pid) -> Result<(), ActorError> {
    let state = ctx.system().state();
    Self::handle_system_terminated(&state);
    Ok(())
  }

  fn supervisor_strategy(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
  ) -> crate::core::supervision::SupervisorStrategy {
    crate::core::supervision::SupervisorStrategy::new(
      crate::core::supervision::SupervisorStrategyKind::OneForOne,
      0,
      core::time::Duration::from_secs(0),
      |_| crate::core::supervision::SupervisorDirective::Stop,
    )
  }
}
