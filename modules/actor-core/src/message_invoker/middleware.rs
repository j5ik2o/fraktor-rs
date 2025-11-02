//! Middleware invoked around actor message handlers.

use crate::{ActorContext, ActorError, AnyMessageView, RuntimeToolbox};

/// Middleware hook executed before and after user message handling.
pub trait MessageInvokerMiddleware<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Called before the actor receives the message.
  ///
  /// # Errors
  ///
  /// Returning an error aborts message processing.
  fn before_user(&self, _ctx: &mut ActorContext<'_, TB>, _message: &AnyMessageView<'_, TB>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called after the actor has processed the message.
  ///
  /// # Errors
  ///
  /// Returning an error replaces the original actor result.
  fn after_user(
    &self,
    _ctx: &mut ActorContext<'_, TB>,
    _message: &AnyMessageView<'_, TB>,
    result: Result<(), ActorError>,
  ) -> Result<(), ActorError> {
    result
  }
}
