use crate::{actor_context::ActorContext, actor_error::ActorError, any_message::AnyMessageView};

/// Middleware invoked before and after actor message handlers.
pub trait MessageInvokerMiddleware: Send + Sync {
  /// Called before invoking the actor with the provided message.
  ///
  /// # Errors
  ///
  /// Returns an error if middleware processing fails. Implementers may return errors
  /// based on message validation, authorization checks, or other pre-processing requirements.
  fn before_user(&self, _ctx: &mut ActorContext<'_>, _message: &AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called after the actor has processed the message. Receives the original result.
  ///
  /// # Errors
  ///
  /// Returns an error if post-processing fails or propagates the original error from the actor.
  /// Implementers may return errors based on result validation, logging failures, or cleanup
  /// issues.
  fn after_user(
    &self,
    _ctx: &mut ActorContext<'_>,
    _message: &AnyMessageView<'_>,
    result: Result<(), ActorError>,
  ) -> Result<(), ActorError> {
    result
  }
}
