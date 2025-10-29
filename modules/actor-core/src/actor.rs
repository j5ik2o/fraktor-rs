//! Core actor trait executed by the runtime.

use crate::{actor_context::ActorContext, actor_error::ActorError, any_message::AnyMessage};

/// Core actor trait executed by the runtime.
///
/// Implementations handle lifecycle callbacks and process incoming [`AnyMessage`] payloads. Each
/// callback returns [`Result<(), ActorError>`] to communicate recoverable or fatal failures to the
/// supervisor tree.
pub trait Actor {
  /// Invoked before the actor starts processing messages.
  ///
  /// Implementations can allocate resources or schedule initial work. Returning a recoverable
  /// error instructs the supervisor to retry the start; a fatal error aborts the actor.
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Handles a single incoming message.
  ///
  /// Message payloads are provided as [`AnyMessage`] values, enabling dynamic downcasting to the
  /// expected type. Errors determine whether the supervisor restarts the actor or escalates the
  /// failure.
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: &AnyMessage<'_>) -> Result<(), ActorError>;

  /// Invoked when the actor is stopping permanently.
  ///
  /// Implementations should release resources and emit necessary termination signals. Failures are
  /// treated as fatal because shutdown is already in progress.
  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}
