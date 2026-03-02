//! Shared wrapper for `ClusterPubSub` implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::cluster_pub_sub::ClusterPubSub;

/// Shared wrapper enabling interior mutability for [`ClusterPubSub`].
///
/// This adapter wraps a pub/sub implementation in a `RuntimeMutex`, allowing
/// callers to access mutable methods via [`SharedAccess`] without requiring a
/// mutable handle to the wrapper itself.
pub struct ClusterPubSubShared {
  inner: ArcShared<RuntimeMutex<Box<dyn ClusterPubSub>>>,
}

impl ClusterPubSubShared {
  /// Creates a new shared wrapper around the given pub/sub implementation.
  #[must_use]
  pub fn new(pub_sub: Box<dyn ClusterPubSub>) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(pub_sub)) }
  }

  /// Creates a wrapper from an existing shared mutex.
  #[must_use]
  pub fn from_inner(inner: ArcShared<RuntimeMutex<Box<dyn ClusterPubSub>>>) -> Self {
    Self { inner }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<RuntimeMutex<Box<dyn ClusterPubSub>>> {
    self.inner.clone()
  }
}

impl Clone for ClusterPubSubShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn ClusterPubSub>> for ClusterPubSubShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ClusterPubSub>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ClusterPubSub>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
