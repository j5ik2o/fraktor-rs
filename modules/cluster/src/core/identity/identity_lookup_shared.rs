//! Shared wrapper for `IdentityLookup` implementations.

use alloc::boxed::Box;
use core::marker::PhantomData;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeMutex, RuntimeToolbox},
  sync::{ArcShared, SharedAccess},
};

use super::identity_lookup::IdentityLookup;

/// Shared wrapper enabling interior mutability for [`IdentityLookup`].
///
/// This adapter wraps an `IdentityLookup` in a `RuntimeMutex`, allowing callers
/// to access mutable methods via [`SharedAccess`] without requiring a mutable
/// handle to the wrapper itself.
pub struct IdentityLookupShared<TB: RuntimeToolbox + 'static> {
  inner:   ArcShared<RuntimeMutex<Box<dyn IdentityLookup>>>,
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> IdentityLookupShared<TB> {
  /// Creates a new shared wrapper around the given identity lookup.
  #[must_use]
  pub fn new(identity_lookup: Box<dyn IdentityLookup>) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(identity_lookup)), _marker: PhantomData }
  }

  /// Creates a wrapper from an existing shared mutex.
  #[must_use]
  pub fn from_inner(inner: ArcShared<RuntimeMutex<Box<dyn IdentityLookup>>>) -> Self {
    Self { inner, _marker: PhantomData }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<RuntimeMutex<Box<dyn IdentityLookup>>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox> Clone for IdentityLookupShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
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
