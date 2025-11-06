//! Typed actor lifecycle contract.

use cellactor_utils_core_rs::sync::NoStdToolbox;

use crate::{
  RuntimeToolbox, actor_prim::Pid, error::ActorError, typed::actor_prim::actor_context::TypedActorContextGeneric,
};

/// Defines the lifecycle hooks for actors that operate on a typed message `M`.
pub trait TypedActor<M, TB = NoStdToolbox>: Send + Sync
where
  TB: RuntimeToolbox + 'static,
  M: Send + Sync + 'static, {
  /// Called before the actor starts processing messages.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor fails to initialize and should not start.
  #[allow(unused_variables)]
  fn pre_start(&mut self, ctx: &mut TypedActorContextGeneric<'_, M, TB>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Handles a typed message dispatched to this actor.
  ///
  /// # Errors
  ///
  /// Returns an error to signal recoverable or fatal processing failures.
  fn receive(&mut self, ctx: &mut TypedActorContextGeneric<'_, M, TB>, message: &M) -> Result<(), ActorError>;

  /// Called after the actor stops.
  ///
  /// # Errors
  ///
  /// Returns an error when cleanup work fails.
  #[allow(unused_variables)]
  fn post_stop(&mut self, ctx: &mut TypedActorContextGeneric<'_, M, TB>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called when a watched actor terminates.
  ///
  /// # Errors
  ///
  /// Returns an error when cleanup logic fails.
  #[allow(unused_variables)]
  fn on_terminated(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    terminated: Pid,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}
