//! Shared wrapper for `Gossiper` implementations.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, SpinSyncMutex};

use super::Gossiper;

/// Shared wrapper enabling interior mutability for [`Gossiper`].
///
/// This adapter wraps a gossiper in a `SharedLock`, allowing callers to
/// access mutable methods via [`SharedAccess`] without requiring a mutable
/// handle to the wrapper itself.
pub struct GossiperShared {
  inner: SharedLock<Box<dyn Gossiper>>,
}

impl GossiperShared {
  /// Creates a new shared wrapper around the given gossiper.
  #[must_use]
  pub fn new(gossiper: Box<dyn Gossiper>) -> Self {
    Self { inner: SharedLock::new_with_driver::<SpinSyncMutex<_>>(gossiper) }
  }
}

impl Clone for GossiperShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn Gossiper>> for GossiperShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn Gossiper>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn Gossiper>) -> R) -> R {
    self.inner.with_write(f)
  }
}
