//! Actor trait definition.

use alloc::boxed::Box;

use crate::{actor_context::ActorContext, actor_error::ActorError, any_message::AnyMessage};

/// Core trait implemented by all actors executed inside the runtime.
pub trait Actor: Send {
  /// Invoked before the actor starts processing messages.
  ///
  /// # Errors
  ///
  /// Returns an error if actor initialization fails.
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Handles an incoming message.
  ///
  /// # Errors
  ///
  /// Returns an error if message processing fails.
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessage<'_>) -> Result<(), ActorError>;

  /// Invoked after the actor has stopped.
  ///
  /// # Errors
  ///
  /// Returns an error if cleanup fails.
  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

impl<T> Actor for Box<T>
where
  T: Actor + ?Sized,
{
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    (**self).pre_start(ctx)
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessage<'_>) -> Result<(), ActorError> {
    (**self).receive(ctx, message)
  }

  fn post_stop(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    (**self).post_stop(ctx)
  }
}
