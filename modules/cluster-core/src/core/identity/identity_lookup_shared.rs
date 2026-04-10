//! Shared wrapper for `IdentityLookup` implementations.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, SpinSyncMutex};

use super::identity_lookup::IdentityLookup;

/// Shared wrapper enabling interior mutability for [`IdentityLookup`].
///
/// This adapter wraps an `IdentityLookup` in a `SharedLock`, allowing callers
/// to access mutable methods via [`SharedAccess`] without requiring a mutable
/// handle to the wrapper itself.
pub struct IdentityLookupShared {
  inner: SharedLock<Box<dyn IdentityLookup>>,
}

impl IdentityLookupShared {
  /// Creates a new shared wrapper around the given identity lookup.
  #[must_use]
  pub fn new(identity_lookup: Box<dyn IdentityLookup>) -> Self {
    Self { inner: SharedLock::new_with_driver::<SpinSyncMutex<_>>(identity_lookup) }
  }
}

impl Clone for IdentityLookupShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn IdentityLookup>> for IdentityLookupShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn IdentityLookup>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn IdentityLookup>) -> R) -> R {
    self.inner.with_write(f)
  }
}
