//! Shared wrapper for `IdentityLookup` implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::identity_lookup::IdentityLookup;

/// Shared wrapper enabling interior mutability for [`IdentityLookup`].
///
/// This adapter wraps an `IdentityLookup` in a `RuntimeMutex`, allowing callers
/// to access mutable methods via [`SharedAccess`] without requiring a mutable
/// handle to the wrapper itself.
pub struct IdentityLookupShared {
  inner: ArcShared<RuntimeMutex<Box<dyn IdentityLookup>>>,
}

impl IdentityLookupShared {
  /// Creates a new shared wrapper around the given identity lookup.
  #[must_use]
  pub fn new(identity_lookup: Box<dyn IdentityLookup>) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(identity_lookup)) }
  }

  /// Creates a wrapper from an existing shared mutex.
  #[must_use]
  pub fn from_inner(inner: ArcShared<RuntimeMutex<Box<dyn IdentityLookup>>>) -> Self {
    Self { inner }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<RuntimeMutex<Box<dyn IdentityLookup>>> {
    self.inner.clone()
  }
}

impl Clone for IdentityLookupShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn IdentityLookup>> for IdentityLookupShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn IdentityLookup>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn IdentityLookup>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
