//! Bounded stash helper for typed behaviors.

#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use crate::core::{error::ActorError, typed::actor::TypedActorContext};

/// Bounded stash helper inspired by Pekko's `StashBuffer`.
pub struct StashBuffer<M>
where
  M: Send + Sync + 'static, {
  capacity: usize,
  _marker:  PhantomData<fn() -> M>,
}

impl<M> StashBuffer<M>
where
  M: Send + Sync + 'static,
{
  /// Creates a stash buffer wrapper with the specified maximum capacity.
  #[must_use]
  pub(crate) const fn new(capacity: usize) -> Self {
    Self { capacity, _marker: PhantomData }
  }

  /// Returns the configured maximum number of stashed messages.
  #[must_use]
  pub const fn capacity(&self) -> usize {
    self.capacity
  }

  /// Returns the current number of stashed messages for the running actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cell is unavailable.
  pub fn len(&self, ctx: &TypedActorContext<'_, M>) -> Result<usize, ActorError> {
    let cell = ctx
      .system()
      .state()
      .cell(&ctx.pid())
      .ok_or_else(|| ActorError::recoverable("actor cell unavailable during stash buffer access"))?;
    Ok(cell.stashed_message_len())
  }

  /// Returns true when no messages are currently stashed.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cell is unavailable.
  pub fn is_empty(&self, ctx: &TypedActorContext<'_, M>) -> Result<bool, ActorError> {
    Ok(self.len(ctx)? == 0)
  }

  /// Returns true when the stash reached its configured capacity.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cell is unavailable.
  pub fn is_full(&self, ctx: &TypedActorContext<'_, M>) -> Result<bool, ActorError> {
    Ok(self.len(ctx)? >= self.capacity)
  }

  /// Returns true when at least one message is currently stashed.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cell is unavailable.
  pub fn is_not_empty(&self, ctx: &TypedActorContext<'_, M>) -> Result<bool, ActorError> {
    Ok(!self.is_empty(ctx)?)
  }

  /// Stashes the currently processed message.
  ///
  /// # Errors
  ///
  /// Returns an error when the stash capacity is reached or when context stashing fails.
  pub fn stash(&self, ctx: &TypedActorContext<'_, M>) -> Result<(), ActorError> {
    ctx.stash_with_limit(self.capacity)
  }

  /// Re-enqueues all stashed messages back to the mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when unstash dispatch fails.
  pub fn unstash_all(&self, ctx: &TypedActorContext<'_, M>) -> Result<usize, ActorError> {
    ctx.unstash_all()
  }
}

impl<M> Clone for StashBuffer<M>
where
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    *self
  }
}

impl<M> Copy for StashBuffer<M> where M: Send + Sync + 'static {}
