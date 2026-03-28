//! Bounded stash helper for typed behaviors.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::core::{
  kernel::{error::ActorError, messaging::AnyMessage},
  typed::actor::TypedActorContext,
};

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
    Self::with_cell(ctx, |cell| cell.stashed_message_len())
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
  pub fn stash(&self, ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    ctx.stash_with_limit(self.capacity)
  }

  /// Returns the oldest stashed message without removing it.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cell is unavailable or the stash is empty.
  pub fn head(&self, ctx: &TypedActorContext<'_, M>) -> Result<M, ActorError>
  where
    M: Clone, {
    Self::with_cell(ctx, |cell| {
      cell.with_stashed_messages(|messages| {
        messages.front().and_then(|message| message.payload().downcast_ref::<M>()).cloned()
      })
    })?
    .ok_or_else(|| ActorError::recoverable("stash buffer is empty"))
  }

  /// Returns true when the stash contains `message`.
  /// Comparison is evaluated against a cloned snapshot so `PartialEq` does not run while the
  /// underlying stash lock is held.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cell is unavailable.
  pub fn contains(&self, ctx: &TypedActorContext<'_, M>, message: &M) -> Result<bool, ActorError>
  where
    M: Clone + PartialEq, {
    self.exists(ctx, |candidate| candidate == message)
  }

  /// Returns true when the predicate matches at least one stashed message.
  /// Matching runs against a cloned snapshot so user predicates are evaluated outside the
  /// underlying stash lock.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cell is unavailable.
  pub fn exists<F>(&self, ctx: &TypedActorContext<'_, M>, mut predicate: F) -> Result<bool, ActorError>
  where
    M: Clone,
    F: FnMut(&M) -> bool, {
    let snapshot = Self::snapshot_stashed_messages(ctx)?;
    Ok(snapshot.iter().any(&mut predicate))
  }

  /// Applies `f` to every stashed message without removing them.
  /// Iteration uses a cloned snapshot so callbacks are invoked outside the underlying stash lock.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cell is unavailable.
  pub fn foreach<F>(&self, ctx: &TypedActorContext<'_, M>, mut f: F) -> Result<(), ActorError>
  where
    M: Clone,
    F: FnMut(&M), {
    let snapshot = Self::snapshot_stashed_messages(ctx)?;
    for message in &snapshot {
      f(message);
    }
    Ok(())
  }

  /// Drops all stashed messages.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cell is unavailable.
  pub fn clear(&self, ctx: &TypedActorContext<'_, M>) -> Result<(), ActorError> {
    Self::with_cell(ctx, |cell| {
      let _cleared_count = cell.clear_stashed_messages();
    })
  }

  /// Re-enqueues all stashed messages back to the mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when unstash dispatch fails.
  pub fn unstash_all(&self, ctx: &TypedActorContext<'_, M>) -> Result<usize, ActorError> {
    ctx.unstash_all()
  }

  /// Re-enqueues at most `count` stashed messages after applying `wrap`.
  ///
  /// # Errors
  ///
  /// Returns an error when actor cell access or unstash dispatch fails.
  pub fn unstash<F>(&self, ctx: &TypedActorContext<'_, M>, count: usize, mut wrap: F) -> Result<usize, ActorError>
  where
    M: Clone,
    F: FnMut(M) -> M, {
    Self::with_cell(ctx, |cell| {
      cell.unstash_messages_with_limit(count, |message| {
        let payload = message
          .payload()
          .downcast_ref::<M>()
          .cloned()
          .ok_or_else(|| ActorError::recoverable("stashed message type mismatch"))?;
        let sender = message.sender().cloned();
        let wrapped = AnyMessage::new(wrap(payload));
        Ok(match sender {
          | Some(sender) => wrapped.with_sender(sender),
          | None => wrapped,
        })
      })
    })?
  }

  fn with_cell<R>(
    ctx: &TypedActorContext<'_, M>,
    f: impl FnOnce(&crate::core::kernel::actor::ActorCell) -> R,
  ) -> Result<R, ActorError> {
    let cell = ctx
      .system()
      .state()
      .cell(&ctx.pid())
      .ok_or_else(|| ActorError::recoverable("actor cell unavailable during stash buffer access"))?;
    Ok(f(&cell))
  }

  fn snapshot_stashed_messages(ctx: &TypedActorContext<'_, M>) -> Result<Vec<M>, ActorError>
  where
    M: Clone, {
    Self::with_cell(ctx, |cell| {
      cell.with_stashed_messages(|messages| {
        messages.iter().filter_map(|message| message.payload().downcast_ref::<M>().cloned()).collect::<Vec<M>>()
      })
    })
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
