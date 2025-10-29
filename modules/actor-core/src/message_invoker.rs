//! Message invoker responsible for executing actor handlers.

use crate::{actor::Actor, actor_context::ActorContext, actor_error::ActorError, any_owned_message::AnyOwnedMessage};

/// Executes actor callbacks with optional middleware support.
pub struct MessageInvoker;

impl MessageInvoker {
  /// Creates a new message invoker without middleware.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  /// Invokes the actor's `receive` handler with the provided message.
  pub fn invoke(
    &self,
    actor: &mut dyn Actor,
    ctx: &mut ActorContext<'_>,
    message: &AnyOwnedMessage,
  ) -> Result<(), ActorError> {
    let borrowed = message.borrow();
    actor.receive(ctx, borrowed)
  }
}

impl Default for MessageInvoker {
  fn default() -> Self {
    Self::new()
  }
}
