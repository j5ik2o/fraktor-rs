//! Actor trait definitions.

use crate::{actor_context::ActorContext, actor_error::ActorError, any_message::AnyMessage};

/// Core lifecycle contract that every actor must implement.
///
/// The runtime invokes the lifecycle hooks in the following order:
/// `pre_start` → repeated `receive` calls → `post_stop` when the actor is shutting down.
/// Returning [`ActorError::Recoverable`] or [`ActorError::Fatal`] allows the supervisor
/// to classify failures according to the configured strategy.
pub trait Actor {
  /// Invoked once before the actor processes the first message.
  ///
  /// The default implementation is a no-op that reports success.
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Handles a single message delivered to the actor.
  ///
  /// Implementors should perform message-specific logic and return [`Ok(())`] when the
  /// message is handled successfully. Returning an [`ActorError`] delegates failure
  /// handling to the parent supervisor.
  fn receive(&mut self, ctx: &mut ActorContext<'_>, msg: AnyMessage<'_>) -> Result<(), ActorError>;

  /// Invoked right before the actor is removed from the system.
  ///
  /// The default implementation is a no-op that reports success.
  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}
