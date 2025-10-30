use alloc::vec::Vec;

use cellactor_utils_core_rs::sync::ArcShared;

use super::message_invoker_middleware::MessageInvokerMiddleware;
use crate::{
  actor::Actor,
  actor_context::ActorContext,
  actor_error::ActorError,
  any_message::{AnyMessage, AnyOwnedMessage},
};

/// Middleware-enabled pipeline used to invoke actor message handlers.
pub struct MessageInvokerPipeline {
  user_middlewares: Vec<ArcShared<dyn MessageInvokerMiddleware>>,
}

impl MessageInvokerPipeline {
  /// Creates a pipeline without any middleware.
  #[must_use]
  pub const fn new() -> Self {
    Self { user_middlewares: Vec::new() }
  }

  /// Builds a pipeline from the provided middleware list.
  #[must_use]
  pub fn from_middlewares(middlewares: Vec<ArcShared<dyn MessageInvokerMiddleware>>) -> Self {
    Self { user_middlewares: middlewares }
  }

  /// Invokes the actor using the configured middleware chain.
  pub fn invoke_user<A>(
    &self,
    actor: &mut A,
    ctx: &mut ActorContext<'_>,
    message: AnyOwnedMessage,
  ) -> Result<(), ActorError>
  where
    A: Actor, {
    let previous = ctx.reply_to().cloned();
    let reply_target = message.reply_to().cloned();

    match reply_target {
      | Some(target) => ctx.set_reply_to(Some(target)),
      | None => ctx.clear_reply_to(),
    }

    if let Err(error) = self.invoke_before(ctx, &message.as_any()) {
      restore_reply(ctx, previous);
      return Err(error);
    }

    let mut result = actor.receive(ctx, message.as_any());

    result = self.invoke_after(ctx, &message.as_any(), result);

    restore_reply(ctx, previous);
    result
  }

  fn invoke_before(&self, ctx: &mut ActorContext<'_>, message: &AnyMessage<'_>) -> Result<(), ActorError> {
    for middleware in &self.user_middlewares {
      middleware.before_user(ctx, message)?;
    }
    Ok(())
  }

  fn invoke_after(
    &self,
    ctx: &mut ActorContext<'_>,
    message: &AnyMessage<'_>,
    mut result: Result<(), ActorError>,
  ) -> Result<(), ActorError> {
    for middleware in self.user_middlewares.iter().rev() {
      result = middleware.after_user(ctx, message, result);
    }
    result
  }
}

fn restore_reply(ctx: &mut ActorContext<'_>, previous: Option<crate::actor_ref::ActorRef>) {
  match previous {
    | Some(target) => ctx.set_reply_to(Some(target)),
    | None => ctx.clear_reply_to(),
  }
}

impl Default for MessageInvokerPipeline {
  fn default() -> Self {
    Self::new()
  }
}
