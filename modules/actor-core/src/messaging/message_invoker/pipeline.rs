//! Middleware-enabled pipeline for invoking actors.

use alloc::vec::Vec;

use cellactor_utils_core_rs::sync::ArcShared;

use super::MessageInvokerMiddleware;
use crate::{
  RuntimeToolbox,
  actor_prim::{Actor, ActorContext, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::{AnyMessageGeneric, any_message_view::AnyMessageView},
};

/// Middleware-enabled pipeline used to invoke actor message handlers.
pub struct MessageInvokerPipeline<TB: RuntimeToolbox + 'static> {
  user_middlewares: Vec<ArcShared<dyn MessageInvokerMiddleware<TB>>>,
}

impl<TB: RuntimeToolbox + 'static> MessageInvokerPipeline<TB> {
  /// Creates a pipeline without any middleware.
  #[must_use]
  pub const fn new() -> Self {
    Self { user_middlewares: Vec::new() }
  }

  /// Builds a pipeline from the provided middleware list.
  #[must_use]
  pub fn from_middlewares(middlewares: Vec<ArcShared<dyn MessageInvokerMiddleware<TB>>>) -> Self {
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
    ctx: &mut ActorContext<'_, TB>,
    message: AnyMessageGeneric<TB>,
  ) -> Result<(), ActorError>
  where
    A: Actor<TB>, {
    let previous = ctx.reply_to().cloned();
    let reply_target = message.reply_to().cloned();

    match reply_target {
      | Some(target) => ctx.set_reply_to(Some(target)),
      | None => ctx.clear_reply_to(),
    }

    let view = message.as_view();

    if let Err(error) = self.invoke_before(ctx, &view) {
      restore_reply(ctx, previous);
      return Err(error);
    }

    let mut result = actor.receive(ctx, view);

    let view_after = message.as_view();
    result = self.invoke_after(ctx, &view_after, result);

    restore_reply(ctx, previous);
    result
  }

  fn invoke_before(&self, ctx: &mut ActorContext<'_, TB>, message: &AnyMessageView<'_, TB>) -> Result<(), ActorError> {
    for middleware in &self.user_middlewares {
      middleware.before_user(ctx, message)?;
    }
    Ok(())
  }

  fn invoke_after(
    &self,
    ctx: &mut ActorContext<'_, TB>,
    message: &AnyMessageView<'_, TB>,
    mut result: Result<(), ActorError>,
  ) -> Result<(), ActorError> {
    for middleware in self.user_middlewares.iter().rev() {
      result = middleware.after_user(ctx, message, result);
    }
    result
  }
}

fn restore_reply<TB: RuntimeToolbox + 'static>(ctx: &mut ActorContext<'_, TB>, previous: Option<ActorRefGeneric<TB>>) {
  match previous {
    | Some(target) => ctx.set_reply_to(Some(target)),
    | None => ctx.clear_reply_to(),
  }
}

impl<TB: RuntimeToolbox + 'static> Default for MessageInvokerPipeline<TB> {
  fn default() -> Self {
    Self::new()
  }
}
