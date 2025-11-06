//! Executes typed behaviors inside the actor runtime.

use cellactor_utils_core_rs::sync::NoStdToolbox;

use crate::{
  RuntimeToolbox,
  error::ActorError,
  typed::{
    actor_prim::{TypedActor, TypedActorContextGeneric},
    behavior::{Behavior, BehaviorDirective},
    behavior_signal::BehaviorSignal,
  },
};

/// Bridges [`Behavior`] objects with the [`TypedActor`] lifecycle.
pub(crate) struct BehaviorRunner<M, TB = NoStdToolbox>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  current:  Behavior<M, TB>,
  stopping: bool,
}

impl<M, TB> BehaviorRunner<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a runner with the provided initial behavior.
  pub(crate) const fn new(initial: Behavior<M, TB>) -> Self {
    Self { current: initial, stopping: false }
  }

  fn apply_transition(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    next: Behavior<M, TB>,
  ) -> Result<(), ActorError> {
    match next.directive() {
      | BehaviorDirective::Same | BehaviorDirective::Ignore => Ok(()),
      | BehaviorDirective::Stopped => {
        if !self.stopping {
          ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))?;
          self.stopping = true;
        }
        self.current = Behavior::stopped();
        Ok(())
      },
      | BehaviorDirective::Active => {
        self.current = next;
        Ok(())
      },
    }
  }

  fn dispatch_signal(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    signal: BehaviorSignal,
  ) -> Result<(), ActorError> {
    let next = self.current.handle_signal(ctx, &signal)?;
    self.apply_transition(ctx, next)
  }
}

impl<M, TB> TypedActor<M, TB> for BehaviorRunner<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn pre_start(&mut self, ctx: &mut TypedActorContextGeneric<'_, M, TB>) -> Result<(), ActorError> {
    self.dispatch_signal(ctx, BehaviorSignal::Started)
  }

  fn receive(&mut self, ctx: &mut TypedActorContextGeneric<'_, M, TB>, message: &M) -> Result<(), ActorError> {
    let next = self.current.handle_message(ctx, message)?;
    self.apply_transition(ctx, next)
  }

  fn post_stop(&mut self, ctx: &mut TypedActorContextGeneric<'_, M, TB>) -> Result<(), ActorError> {
    self.dispatch_signal(ctx, BehaviorSignal::Stopped)
  }

  fn on_terminated(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    terminated: crate::actor_prim::Pid,
  ) -> Result<(), ActorError> {
    self.dispatch_signal(ctx, BehaviorSignal::Terminated(terminated))
  }
}
