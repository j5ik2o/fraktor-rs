//! Middleware-enabled pipeline for invoking actors.

use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::SharedAccess;

use super::middleware_shared::MiddlewareShared;
use crate::core::kernel::{
  actor::{Actor, ActorContext, actor_ref::ActorRef},
  error::ActorError,
  messaging::{AnyMessage, any_message_view::AnyMessageView},
};

/// Middleware-enabled pipeline used to invoke actor message handlers.
pub struct MessageInvokerPipeline {
  user_middlewares: Vec<MiddlewareShared>,
}

impl MessageInvokerPipeline {
  /// Creates a pipeline without any middleware.
  #[must_use]
  pub const fn new() -> Self {
    Self { user_middlewares: Vec::new() }
  }

  /// Builds a pipeline from the provided middleware list.
  #[must_use]
  #[allow(dead_code)] // Used in tests
  pub(crate) const fn from_middlewares(middlewares: Vec<MiddlewareShared>) -> Self {
    Self { user_middlewares: middlewares }
  }

  /// Invokes the actor using the configured middleware chain.
  ///
  /// # Errors
  ///
  /// Returns an error if middleware processing fails or if the actor's handler returns an error.
  #[allow(clippy::needless_pass_by_value)]
  pub fn invoke_user<A>(
    &self,
    actor: &mut A,
    ctx: &mut ActorContext<'_>,
    message: AnyMessage,
  ) -> Result<(), ActorError>
  where
    A: Actor + ?Sized, {
    let previous = ctx.sender().cloned();
    let sender = message.sender().cloned();

    match sender {
      | Some(target) => ctx.set_sender(Some(target)),
      | None => ctx.clear_sender(),
    }
    ctx.set_current_message(Some(message.clone()));

    let view = message.as_view();

    if let Err(error) = self.invoke_before(ctx, &view) {
      ctx.clear_current_message();
      restore_sender(ctx, previous);
      return Err(error);
    }

    let mut result = actor.receive(ctx, view);

    let view_after = message.as_view();
    result = self.invoke_after(ctx, &view_after, result);

    ctx.clear_current_message();
    restore_sender(ctx, previous);
    result
  }

  fn invoke_before(&self, ctx: &mut ActorContext<'_>, message: &AnyMessageView<'_>) -> Result<(), ActorError> {
    for middleware in &self.user_middlewares {
      middleware.with_write(|m| m.before_user(ctx, message))?;
    }
    Ok(())
  }

  fn invoke_after(
    &self,
    ctx: &mut ActorContext<'_>,
    message: &AnyMessageView<'_>,
    mut result: Result<(), ActorError>,
  ) -> Result<(), ActorError> {
    for middleware in self.user_middlewares.iter().rev() {
      result = middleware.with_write(|m| m.after_user(ctx, message, result));
    }
    result
  }
}

fn restore_sender(ctx: &mut ActorContext<'_>, previous: Option<ActorRef>) {
  match previous {
    | Some(target) => ctx.set_sender(Some(target)),
    | None => ctx.clear_sender(),
  }
}

impl Default for MessageInvokerPipeline {
  fn default() -> Self {
    Self::new()
  }
}
