//! Typed actor lifecycle contract.

use crate::core::{
  kernel::{
    actor::{
      Pid,
      error::{ActorError, ActorErrorReason},
      supervision::SupervisorStrategyConfig,
    },
    dispatch::mailbox::metrics_event::MailboxPressureEvent,
  },
  typed::{actor::actor_context::TypedActorContext, message_adapter::AdapterError},
};

/// Defines the lifecycle hooks for actors that operate on a typed message `M`.
pub trait TypedActor<M>: Send + Sync
where
  M: Send + Sync + 'static, {
  /// Called before the actor starts processing messages.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor fails to initialize and should not start.
  #[allow(unused_variables)]
  fn pre_start(&mut self, ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Handles a typed message dispatched to this actor.
  ///
  /// # Errors
  ///
  /// Returns an error to signal recoverable or fatal processing failures.
  fn receive(&mut self, ctx: &mut TypedActorContext<'_, M>, message: &M) -> Result<(), ActorError>;

  /// Called after the actor stops.
  ///
  /// # Errors
  ///
  /// Returns an error when cleanup work fails.
  #[allow(unused_variables)]
  fn post_stop(&mut self, ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called when a watched actor terminates.
  ///
  /// # Errors
  ///
  /// Returns an error when cleanup logic fails.
  #[allow(unused_variables)]
  fn on_terminated(&mut self, ctx: &mut TypedActorContext<'_, M>, terminated: Pid) -> Result<(), ActorError> {
    Ok(())
  }

  /// Provides the supervision strategy for this typed actor.
  ///
  /// The actor state is queried immutably and the context is exposed as a
  /// read-only view.
  #[must_use]
  fn supervisor_strategy(&self, _ctx: &TypedActorContext<'_, M>) -> SupervisorStrategyConfig {
    SupervisorStrategyConfig::default()
  }

  /// Called when this actor's mailbox reaches high pressure while messages are queued.
  ///
  /// # Errors
  ///
  /// Returns an error if the actor cannot handle pressure conditions.
  #[allow(unused_variables)]
  fn on_mailbox_pressure(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
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
    _ctx: &mut TypedActorContext<'_, M>,
    _failure: AdapterError,
  ) -> Result<(), ActorError> {
    Err(ActorError::recoverable(ActorErrorReason::new("message adapter failure")))
  }

  /// Called before the actor is restarted by its supervisor.
  ///
  /// The default implementation delegates to [`post_stop`](TypedActor::post_stop).
  ///
  /// # Errors
  ///
  /// Returns an error when pre-restart cleanup fails.
  fn pre_restart(&mut self, ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    self.post_stop(ctx)
  }

  /// Called after the actor has been restarted by its supervisor.
  ///
  /// Pekko `aroundPostRestart` の契約は「`postRestart` 実行後に `preStart` を再度呼ぶ」こと。
  /// `TypedActorAdapter::post_restart` 側がこの 2 段の呼び出しを実行するため、本 trait の
  /// デフォルト実装は何もしない (`Ok(())`)。`preStart` の再実行を抑止したい実装のみが本
  /// メソッドを override する。
  ///
  /// # Errors
  ///
  /// Returns an error when post-restart work fails.
  #[allow(unused_variables)]
  fn post_restart(&mut self, ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called when a supervised child actor fails.
  ///
  /// # Errors
  ///
  /// Returns an error when the notification cannot be processed.
  #[allow(unused_variables)]
  fn on_child_failed(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    child: Pid,
    error: &ActorError,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}
