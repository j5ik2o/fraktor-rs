//! Bounded stash helper for typed behaviors.

#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{actor::STASH_OVERFLOW_REASON, error::ActorError, typed::actor::TypedActorContextGeneric};

/// Bounded stash helper inspired by Pekko's `StashBuffer`.
pub struct StashBufferGeneric<M, TB = NoStdToolbox>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  capacity: usize,
  _marker:  PhantomData<fn() -> (M, TB)>,
}

/// Type alias for [StashBufferGeneric] with the default [NoStdToolbox].
pub type StashBuffer<M> = StashBufferGeneric<M, NoStdToolbox>;

impl<M, TB> StashBufferGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
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
  pub fn len(&self, ctx: &TypedActorContextGeneric<'_, M, TB>) -> Result<usize, ActorError> {
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
  pub fn is_empty(&self, ctx: &TypedActorContextGeneric<'_, M, TB>) -> Result<bool, ActorError> {
    Ok(self.len(ctx)? == 0)
  }

  /// Returns true when the stash reached its configured capacity.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cell is unavailable.
  pub fn is_full(&self, ctx: &TypedActorContextGeneric<'_, M, TB>) -> Result<bool, ActorError> {
    Ok(self.len(ctx)? >= self.capacity)
  }

  /// Stashes the currently processed message.
  ///
  /// # Errors
  ///
  /// Returns an error when the stash capacity is reached or when context stashing fails.
  pub fn stash(&self, ctx: &TypedActorContextGeneric<'_, M, TB>) -> Result<(), ActorError> {
    if self.is_full(ctx)? {
      return Err(ActorError::recoverable(STASH_OVERFLOW_REASON));
    }
    ctx.stash()
  }

  /// Re-enqueues all stashed messages back to the mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when unstash dispatch fails.
  pub fn unstash_all(&self, ctx: &TypedActorContextGeneric<'_, M, TB>) -> Result<usize, ActorError> {
    ctx.unstash_all()
  }
}

impl<M, TB> Clone for StashBufferGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn clone(&self) -> Self {
    *self
  }
}

impl<M, TB> Copy for StashBufferGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
}
