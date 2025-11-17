//! Middleware invoked around actor message handlers.

use fraktor_utils_core_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{actor_prim::ActorContextGeneric, error::ActorError, messaging::AnyMessageViewGeneric};

/// Middleware hook executed before and after user message handling.
pub trait MessageInvokerMiddleware<TB: RuntimeToolbox + 'static = NoStdToolbox>: Send + Sync {
  /// Called before the actor receives the message.
  ///
  /// # Errors
  ///
  /// Returning an error aborts message processing.
  fn before_user(
    &self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: &AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called after the actor has processed the message.
  ///
  /// # Errors
  ///
  /// Returning an error replaces the original actor result.
  fn after_user(
    &self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: &AnyMessageViewGeneric<'_, TB>,
    result: Result<(), ActorError>,
  ) -> Result<(), ActorError> {
    result
  }
}
