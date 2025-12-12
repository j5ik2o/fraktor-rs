//! Shared wrapper for `IdentityLookup` implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use crate::core::IdentityLookup;

/// Shared wrapper enabling interior mutability for [`IdentityLookup`].
///
/// This adapter wraps an `IdentityLookup` in a `ToolboxMutex`, allowing callers
/// to access mutable methods via [`SharedAccess`] without requiring a mutable
/// handle to the wrapper itself.
pub struct IdentityLookupShared<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn IdentityLookup>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> IdentityLookupShared<TB> {
  /// Creates a new shared wrapper around the given identity lookup.
  #[must_use]
  pub fn new(identity_lookup: Box<dyn IdentityLookup>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(identity_lookup)) }
  }

  /// Creates a wrapper from an existing shared mutex.
  #[must_use]
  pub fn from_inner(inner: ArcShared<ToolboxMutex<Box<dyn IdentityLookup>, TB>>) -> Self {
    Self { inner }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<ToolboxMutex<Box<dyn IdentityLookup>, TB>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox> Clone for IdentityLookupShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn IdentityLookup>> for IdentityLookupShared<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn IdentityLookup>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn IdentityLookup>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
