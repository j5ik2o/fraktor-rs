#![allow(clippy::disallowed_types)]
#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Arc;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use core::ptr;

use super::{Shared, SharedDyn};

/// Shared wrapper backed by `alloc::sync::Arc`.
///
/// Targets that lack atomic pointer operations (`target_has_atomic = "ptr"`)
/// do not provide `alloc::sync::Arc`. In those environments we transparently
/// fall back to `alloc::rc::Rc`, allowing higher layers to keep using a unified
/// shared abstraction.
pub struct ArcShared<T: ?Sized>(Arc<T>);

impl<T: ?Sized> ArcShared<T> {
  /// Creates a new `ArcShared` by wrapping the provided value.
  pub fn new(value: T) -> Self
  where
    T: Sized, {
    Self(Arc::new(value))
  }

  /// For Testing, Don't Use Production
  ///
  /// Wraps an existing `Arc` in the shared wrapper.
  #[must_use]
  pub const fn from_arc_for_testing_dont_use_production(inner: Arc<T>) -> Self {
    Self(inner)
  }

  /// For Testing, Don't Use Production
  ///
  /// Consumes the wrapper and returns the inner `Arc`.
  #[must_use]
  pub fn into_arc_for_testing_dont_use_production(self) -> Arc<T> {
    self.0
  }

  /// Consumes the shared handle and returns the raw pointer.
  #[must_use]
  pub fn into_raw(self) -> *const T {
    Arc::into_raw(self.0)
  }

  /// Reconstructs the shared handle from a raw pointer.
  ///
  /// # Safety
  ///
  /// The pointer must originate from `ArcShared::into_raw`.
  pub unsafe fn from_raw(ptr: *const T) -> Self {
    Self(unsafe { Arc::from_raw(ptr) })
  }

  /// Converts the shared handle into another dynamically sized representation.
  pub fn into_dyn<U: ?Sized, F>(self, cast: F) -> ArcShared<U>
  where
    F: FnOnce(&T) -> &U, {
    let raw = self.into_raw();
    unsafe {
      let reference = &*raw;
      let trait_reference = cast(reference);
      let trait_ptr = ptr::from_ref(trait_reference);
      ArcShared::from_raw(trait_ptr)
    }
  }
}

impl<T: ?Sized> core::ops::Deref for ArcShared<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T: ?Sized> Shared<T> for ArcShared<T> {
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Arc::try_unwrap(self.0).map_err(ArcShared)
  }
}

impl<T: ?Sized> SharedDyn<T> for ArcShared<T> {
  type Dyn<U: ?Sized + 'static> = ArcShared<U>;

  fn into_dyn<U: ?Sized + 'static, F>(self, cast: F) -> Self::Dyn<U>
  where
    F: FnOnce(&T) -> &U, {
    ArcShared::into_dyn(self, cast)
  }
}

impl<T: ?Sized> Clone for ArcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}
