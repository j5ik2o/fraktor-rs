//! Pending handler invocation queue entries.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use crate::core::persistent_repr::PersistentRepr;

type PendingHandler<A> = Box<dyn FnOnce(&mut A, &PersistentRepr) + Send + Sync>;

/// Pending invocation stored while persisting.
pub enum PendingHandlerInvocation<A> {
  /// Invocation that stashes incoming commands.
  Stashing {
    /// Persistent representation.
    repr:     PersistentRepr,
    /// Handler callback.
    handler:  PendingHandler<A>,
    /// True when created through defer/defer_async.
    deferred: bool,
  },
  /// Invocation that does not stash commands.
  Async {
    /// Persistent representation.
    repr:     PersistentRepr,
    /// Handler callback.
    handler:  PendingHandler<A>,
    /// True when created through defer/defer_async.
    deferred: bool,
  },
}

impl<A> PendingHandlerInvocation<A> {
  /// Creates a stashing invocation from a boxed handler.
  #[must_use]
  pub fn stashing_boxed(repr: PersistentRepr, handler: PendingHandler<A>) -> Self {
    Self::Stashing { repr, handler, deferred: false }
  }

  /// Creates a deferred stashing invocation from a boxed handler.
  #[must_use]
  pub fn stashing_deferred_boxed(repr: PersistentRepr, handler: PendingHandler<A>) -> Self {
    Self::Stashing { repr, handler, deferred: true }
  }

  /// Creates a stashing invocation.
  #[must_use]
  pub fn stashing(repr: PersistentRepr, handler: impl FnOnce(&mut A, &PersistentRepr) + Send + Sync + 'static) -> Self {
    Self::stashing_boxed(repr, Box::new(handler))
  }

  /// Creates an async invocation from a boxed handler.
  #[must_use]
  pub fn async_handler_boxed(repr: PersistentRepr, handler: PendingHandler<A>) -> Self {
    Self::Async { repr, handler, deferred: false }
  }

  /// Creates a deferred async invocation from a boxed handler.
  #[must_use]
  pub fn async_deferred_boxed(repr: PersistentRepr, handler: PendingHandler<A>) -> Self {
    Self::Async { repr, handler, deferred: true }
  }

  /// Creates an async invocation.
  #[must_use]
  pub fn async_handler(
    repr: PersistentRepr,
    handler: impl FnOnce(&mut A, &PersistentRepr) + Send + Sync + 'static,
  ) -> Self {
    Self::async_handler_boxed(repr, Box::new(handler))
  }

  /// Creates a deferred stashing invocation.
  #[must_use]
  pub fn stashing_deferred(
    repr: PersistentRepr,
    handler: impl FnOnce(&mut A, &PersistentRepr) + Send + Sync + 'static,
  ) -> Self {
    Self::stashing_deferred_boxed(repr, Box::new(handler))
  }

  /// Creates a deferred async invocation.
  #[must_use]
  pub fn async_deferred(
    repr: PersistentRepr,
    handler: impl FnOnce(&mut A, &PersistentRepr) + Send + Sync + 'static,
  ) -> Self {
    Self::async_deferred_boxed(repr, Box::new(handler))
  }

  /// Returns true when the invocation stashes commands.
  #[must_use]
  pub const fn is_stashing(&self) -> bool {
    matches!(self, PendingHandlerInvocation::Stashing { .. })
  }

  /// Returns true when the invocation came from defer/defer_async.
  #[must_use]
  pub const fn is_deferred(&self) -> bool {
    match self {
      | PendingHandlerInvocation::Stashing { deferred, .. } | PendingHandlerInvocation::Async { deferred, .. } => {
        *deferred
      },
    }
  }

  /// Returns the sequence number associated with this invocation.
  #[must_use]
  pub const fn sequence_nr(&self) -> u64 {
    match self {
      | PendingHandlerInvocation::Stashing { repr, .. } | PendingHandlerInvocation::Async { repr, .. } => {
        repr.sequence_nr()
      },
    }
  }

  /// Invokes the handler.
  pub fn invoke(self, actor: &mut A) {
    match self {
      | PendingHandlerInvocation::Stashing { repr, handler, .. } => handler(actor, &repr),
      | PendingHandlerInvocation::Async { repr, handler, .. } => handler(actor, &repr),
    }
  }
}
