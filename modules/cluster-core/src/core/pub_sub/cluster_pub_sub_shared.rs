//! Shared wrapper for `ClusterPubSub` implementations.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, DefaultMutex};

use super::cluster_pub_sub::ClusterPubSub;

/// Shared wrapper enabling interior mutability for [`ClusterPubSub`].
///
/// This adapter wraps a pub/sub implementation in a `SharedLock`, allowing
/// callers to access mutable methods via [`SharedAccess`] without requiring a
/// mutable handle to the wrapper itself.
pub struct ClusterPubSubShared {
  inner: SharedLock<Box<dyn ClusterPubSub>>,
}

impl ClusterPubSubShared {
  /// Creates a new shared wrapper around the given pub/sub implementation.
  #[must_use]
  pub fn new(pub_sub: Box<dyn ClusterPubSub>) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(pub_sub) }
  }
}

impl Clone for ClusterPubSubShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn ClusterPubSub>> for ClusterPubSubShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ClusterPubSub>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ClusterPubSub>) -> R) -> R {
    self.inner.with_write(f)
  }
}
