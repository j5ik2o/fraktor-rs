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
    repr:    PersistentRepr,
    /// Handler callback.
    handler: PendingHandler<A>,
  },
  /// Invocation that does not stash commands.
  Async {
    /// Persistent representation.
    repr:    PersistentRepr,
    /// Handler callback.
    handler: PendingHandler<A>,
  },
}

impl<A> PendingHandlerInvocation<A> {
  /// Creates a stashing invocation.
  #[must_use]
  pub fn stashing(repr: PersistentRepr, handler: impl FnOnce(&mut A, &PersistentRepr) + Send + Sync + 'static) -> Self {
    Self::Stashing { repr, handler: Box::new(handler) }
  }

  /// Creates an async invocation.
  #[must_use]
  pub fn async_handler(
    repr: PersistentRepr,
    handler: impl FnOnce(&mut A, &PersistentRepr) + Send + Sync + 'static,
  ) -> Self {
    Self::Async { repr, handler: Box::new(handler) }
  }

  /// Returns true when the invocation stashes commands.
  #[must_use]
  pub const fn is_stashing(&self) -> bool {
    matches!(self, PendingHandlerInvocation::Stashing { .. })
  }

  /// Invokes the handler.
  pub fn invoke(self, actor: &mut A) {
    match self {
      | PendingHandlerInvocation::Stashing { repr, handler } => handler(actor, &repr),
      | PendingHandlerInvocation::Async { repr, handler } => handler(actor, &repr),
    }
  }
}
