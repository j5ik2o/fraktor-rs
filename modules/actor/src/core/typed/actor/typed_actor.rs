//! Typed actor lifecycle contract.

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{
  actor::Pid,
  dispatch::mailbox::metrics_event::MailboxPressureEvent,
  error::{ActorError, ActorErrorReason},
  supervision::SupervisorStrategy,
  typed::{actor::actor_context::TypedActorContextGeneric, message_adapter::AdapterError},
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

  /// Provides the supervision strategy for this typed actor.
  #[must_use]
  fn supervisor_strategy(&mut self, _ctx: &mut TypedActorContextGeneric<'_, M, TB>) -> SupervisorStrategy {
    SupervisorStrategy::default()
  }

  /// Called when this actor's mailbox reaches high pressure while messages are queued.
  ///
  /// # Errors
  ///
  /// Returns an error if the actor cannot handle pressure conditions.
  #[allow(unused_variables)]
  fn on_mailbox_pressure(
    &mut self,
    ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    event: &MailboxPressureEvent,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called when a message adapter fails before delivering a message.
  ///
  /// # Errors
  ///
  /// Returns an error if the failure cannot be handled.
  fn on_adapter_failure(
    &mut self,
    _ctx: &mut TypedActorContextGeneric<'_, M, TB>,
    _failure: AdapterError,
  ) -> Result<(), ActorError> {
    Err(ActorError::recoverable(ActorErrorReason::new("message adapter failure")))
  }
}
