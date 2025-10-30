use crate::{actor_context::ActorContext, actor_error::ActorError, any_message::AnyMessage};

/// Middleware invoked before and after actor message handlers.
pub trait MessageInvokerMiddleware: Send + Sync {
  /// Called before invoking the actor with the provided message.
  fn before_user(&self, _ctx: &mut ActorContext<'_>, _message: &AnyMessage<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called after the actor has processed the message. Receives the original result.
  fn after_user(
    &self,
    _ctx: &mut ActorContext<'_>,
    _message: &AnyMessage<'_>,
    result: Result<(), ActorError>,
  ) -> Result<(), ActorError> {
    result
  }
}
