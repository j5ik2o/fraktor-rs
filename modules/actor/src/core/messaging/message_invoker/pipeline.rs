//! Middleware-enabled pipeline for invoking actors.

use alloc::vec::Vec;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::SharedAccess,
};

use super::middleware_shared::MiddlewareShared;
use crate::core::{
  actor::{Actor, ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::{AnyMessageGeneric, any_message_view::AnyMessageViewGeneric},
};

/// Middleware-enabled pipeline used to invoke actor message handlers.
pub struct MessageInvokerPipelineGeneric<TB: RuntimeToolbox + 'static> {
  user_middlewares: Vec<MiddlewareShared<TB>>,
}

/// Type alias for [MessageInvokerPipelineGeneric] with the default [NoStdToolbox].
pub type MessageInvokerPipeline = MessageInvokerPipelineGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> MessageInvokerPipelineGeneric<TB> {
  /// Creates a pipeline without any middleware.
  #[must_use]
  pub const fn new() -> Self {
    Self { user_middlewares: Vec::new() }
  }

  /// Builds a pipeline from the provided middleware list.
  #[must_use]
  #[allow(dead_code)] // Used in tests
  pub(crate) const fn from_middlewares(middlewares: Vec<MiddlewareShared<TB>>) -> Self {
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
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageGeneric<TB>,
  ) -> Result<(), ActorError>
  where
    A: Actor<TB> + ?Sized, {
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

  fn invoke_before(
    &self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: &AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    for middleware in &self.user_middlewares {
      middleware.with_write(|m| m.before_user(ctx, message))?;
    }
    Ok(())
  }

  fn invoke_after(
    &self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: &AnyMessageViewGeneric<'_, TB>,
    mut result: Result<(), ActorError>,
  ) -> Result<(), ActorError> {
    for middleware in self.user_middlewares.iter().rev() {
      result = middleware.with_write(|m| m.after_user(ctx, message, result));
    }
    result
  }
}

fn restore_sender<TB: RuntimeToolbox + 'static>(
  ctx: &mut ActorContextGeneric<'_, TB>,
  previous: Option<ActorRefGeneric<TB>>,
) {
  match previous {
    | Some(target) => ctx.set_sender(Some(target)),
    | None => ctx.clear_sender(),
  }
}

impl<TB: RuntimeToolbox + 'static> Default for MessageInvokerPipelineGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
