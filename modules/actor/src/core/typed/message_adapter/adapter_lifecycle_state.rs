//! Tracks the lifecycle of adapter references.

#[cfg(test)]
mod tests;

use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

/// Lifecycle guard shared between adapter handles and senders.
pub(crate) struct AdapterLifecycleState<TB: RuntimeToolbox + 'static> {
  alive:    AtomicBool,
  _toolbox: core::marker::PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> AdapterLifecycleState<TB> {
  /// Creates a new lifecycle state.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self { alive: AtomicBool::new(true), _toolbox: core::marker::PhantomData }
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
