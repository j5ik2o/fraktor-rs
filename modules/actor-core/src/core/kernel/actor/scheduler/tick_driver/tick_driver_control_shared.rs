//! Shared wrapper for tick driver control implementations.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};

use super::TickDriverControl;

/// Shared wrapper that serializes access to a tick-driver control hook.
pub struct TickDriverControlShared {
  inner: SharedLock<Box<dyn TickDriverControl>>,
}

impl TickDriverControlShared {
  /// Creates a new shared wrapper using the builtin spin lock backend.
  #[must_use]
  pub fn new(control: Box<dyn TickDriverControl>) -> Self {
    Self::from_shared_lock(SharedLock::new_with_driver::<DefaultMutex<_>>(control))
  }

  /// Creates a shared wrapper from an existing shared lock.
  #[must_use]
  pub const fn from_shared_lock(inner: SharedLock<Box<dyn TickDriverControl>>) -> Self {
    Self { inner }
  }

  /// Stops the underlying driver control.
  pub fn shutdown(&self) {
    self.inner.with_lock(|control| control.shutdown());
  }
}

impl Clone for TickDriverControlShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
