use crate::{actor_context::ActorContext, actor_error::ActorError, any_message::AnyMessage};

/// Behavior handler invoked for each message.
pub trait ReceiveHandler: Send + Sync {
  /// Handles the provided message using the supplied context.
  fn handle(&self, ctx: &mut ActorContext<'_>, message: &AnyMessage<'_>) -> Result<(), ActorError>;
}
