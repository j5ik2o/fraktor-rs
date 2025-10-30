//! Actor trait definition.

use crate::{actor_context::ActorContext, actor_error::ActorError, any_message::AnyMessage};

/// Core trait implemented by all actors executed inside the runtime.
pub trait Actor: Send {
  /// Invoked before the actor starts processing messages.
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Handles an incoming message.
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessage<'_>) -> Result<(), ActorError>;

  /// Invoked after the actor has stopped.
  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}
