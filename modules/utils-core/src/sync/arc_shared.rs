#[cfg(not(feature = "force-portable-arc"))]
use alloc::sync::Arc;
#[cfg(not(feature = "unsize"))]
use core::ptr;
#[cfg(feature = "unsize")]
use core::{marker::Unsize, ops::CoerceUnsized};

#[cfg(feature = "force-portable-arc")]
pub use portable_atomic_util::Arc;

use super::{Shared, SharedDyn};

#[cfg(test)]
mod tests;

/// Shared wrapper backed by [`alloc::sync::Arc`] by default.
///
/// When the `force-portable-arc` feature is enabled it switches to [`portable_atomic_util::Arc`]
/// so that targets without native atomic pointer support still benefit from an `Arc`-compatible
/// shared handle.
#[repr(transparent)]
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
  pub const fn ___from_arc(inner: Arc<T>) -> Self {
    Self(inner)
  }

  /// For Testing, Don't Use Production
  ///
  /// Consumes the wrapper and returns the inner `Arc`.
  #[must_use]
  pub fn ___into_arc(self) -> Arc<T> {
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
  #[cfg(not(feature = "unsize"))]
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

  /// Converts the shared handle into another dynamically sized representation.
  #[cfg(feature = "unsize")]
  #[deprecated(
    note = "ArcShared::into_dyn is disabled when the `unsize` feature is enabled; rely on implicit coercion instead."
  )]
  /// # Panics
  ///
  /// Always panics because the `unsize` feature enables implicit coercion and this method must not
  /// be used directly.
  pub fn into_dyn<U: ?Sized, F>(self, cast: F) -> ArcShared<U>
  where
    F: FnOnce(&T) -> &U, {
    let _ = cast;
    panic!("ArcShared::into_dyn is disabled when the `unsize` feature is enabled; rely on implicit coercion instead.");
  }
}

impl<T: ?Sized> core::ops::Deref for ArcShared<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T: ?Sized + core::fmt::Debug> core::fmt::Debug for ArcShared<T> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("ArcShared").finish()
  }
}

impl<T: ?Sized> PartialEq for ArcShared<T> {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl<T: ?Sized> Eq for ArcShared<T> {}

impl<T: ?Sized> Shared<T> for ArcShared<T> {
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Arc::try_unwrap(self.0).map_err(ArcShared)
  }
}

#[cfg(not(feature = "unsize"))]
impl<T: ?Sized> SharedDyn<T> for ArcShared<T> {
  type Dyn<U: ?Sized + 'static> = ArcShared<U>;

  fn into_dyn<U: ?Sized + 'static, F>(self, cast: F) -> Self::Dyn<U>
  where
    F: FnOnce(&T) -> &U, {
    ArcShared::into_dyn(self, cast)
  }
}

#[cfg(feature = "unsize")]
impl<T: ?Sized> SharedDyn<T> for ArcShared<T> {
  type Dyn<U: ?Sized + 'static> = ArcShared<U>;

  fn into_dyn<U: ?Sized + 'static, F>(self, cast: F) -> Self::Dyn<U>
  where
    F: FnOnce(&T) -> &U, {
    #[allow(deprecated)]
    {
      self.into_dyn(cast)
    }
  }
}

impl<T: ?Sized> Clone for ArcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

#[cfg(feature = "unsize")]
impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<ArcShared<U>> for ArcShared<T> {}
