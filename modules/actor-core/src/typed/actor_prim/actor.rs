//! Typed actor lifecycle contract.

use crate::{
  RuntimeToolbox, actor_prim::Pid, error::ActorError, typed::actor_prim::actor_context::TypedActorContextGeneric,
};

/// Defines the lifecycle hooks for actors that operate on a typed message `M`.
pub trait TypedActor<TB, M>: Send + Sync
where
  TB: RuntimeToolbox + 'static,
  M: Send + Sync + 'static, {
  /// Called before the actor starts processing messages.
  #[allow(unused_variables)]
  fn pre_start(&mut self, ctx: &mut TypedActorContextGeneric<'_, TB, M>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Handles a typed message dispatched to this actor.
  fn receive(&mut self, ctx: &mut TypedActorContextGeneric<'_, TB, M>, message: &M) -> Result<(), ActorError>;

  /// Called after the actor stops.
  #[allow(unused_variables)]
  fn post_stop(&mut self, ctx: &mut TypedActorContextGeneric<'_, TB, M>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called when a watched actor terminates.
  #[allow(unused_variables)]
  fn on_terminated(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, TB, M>,
    terminated: Pid,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}
