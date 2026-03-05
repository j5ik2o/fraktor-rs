//! Internal root guardian that supervises `/user` and `/system`.

use crate::core::{
  actor::{Actor, ActorContext, Pid, actor_ref::ActorRef},
  error::ActorError,
  messaging::AnyMessageView,
  supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyConfig, SupervisorStrategyKind},
  system::state::SystemStateShared,
};

/// Root guardian actor responsible for watching the system guardian.
pub(crate) struct RootGuardianActor;

impl RootGuardianActor {
  /// Creates a new root guardian instance.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self
  }

  fn watch_system_guardian(ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    if let Some(cell) = ctx.system().state().system_guardian() {
      let system_ref: ActorRef = cell.actor_ref();
      ctx.watch(&system_ref).map_err(|error| ActorError::from_send_error(&error))
    } else {
      Ok(())
    }
  }

  fn handle_system_terminated(state: &SystemStateShared) {
    state.mark_terminated();
  }
}

impl Actor for RootGuardianActor {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Self::watch_system_guardian(ctx)
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn on_terminated(&mut self, ctx: &mut ActorContext<'_>, _terminated: Pid) -> Result<(), ActorError> {
    let state = ctx.system().state();
    Self::handle_system_terminated(&state);
    Ok(())
  }

  fn supervisor_strategy(&mut self, _ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 0, core::time::Duration::from_secs(0), |_| {
      SupervisorDirective::Stop
    })
    .into()
  }
}
