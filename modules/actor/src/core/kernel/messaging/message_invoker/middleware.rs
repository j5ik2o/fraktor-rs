//! Middleware invoked around actor message handlers.

use crate::core::kernel::{actor::ActorContext, error::ActorError, messaging::AnyMessageView};

/// Middleware hook executed before and after user message handling.
pub trait MessageInvokerMiddleware: Send + Sync {
  /// Called before the actor receives the message.
  ///
  /// # Errors
  ///
  /// Returning an error aborts message processing.
  fn before_user(&mut self, _ctx: &mut ActorContext<'_>, _message: &AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called after the actor has processed the message.
  ///
  /// # Errors
  ///
  /// Returning an error replaces the original actor result.
  fn after_user(
    &mut self,
    _ctx: &mut ActorContext<'_>,
    _message: &AnyMessageView<'_>,
    result: Result<(), ActorError>,
  ) -> Result<(), ActorError> {
    result
  }
}
