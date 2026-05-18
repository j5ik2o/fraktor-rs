//! Tracks the lifecycle of adapter references.

#[cfg(test)]
#[path = "adapter_lifecycle_state_test.rs"]
mod tests;

use core::{
  marker::PhantomData,
  sync::atomic::{AtomicBool, Ordering},
};

/// Lifecycle guard shared between adapter handles and senders.
pub(crate) struct AdapterLifecycleState {
  alive:    AtomicBool,
  _toolbox: PhantomData<()>,
}

impl AdapterLifecycleState {
  /// Creates a new lifecycle state.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self { alive: AtomicBool::new(true), _toolbox: PhantomData }
  }

  /// Returns whether the adapter is still alive.
  #[must_use]
  pub(crate) fn is_alive(&self) -> bool {
    self.alive.load(Ordering::SeqCst)
  }

  /// Marks the adapter as stopped.
  pub(crate) fn mark_stopped(&self) {
    self.alive.store(false, Ordering::SeqCst);
  }
}
