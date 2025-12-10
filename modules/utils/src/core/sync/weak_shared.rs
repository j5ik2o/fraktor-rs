//! Weak reference wrapper for shared ownership.

#[cfg(not(feature = "force-portable-arc"))]
use alloc::sync::Weak;

#[cfg(feature = "force-portable-arc")]
use portable_atomic_util::Weak;

use super::arc_shared::ArcShared;

/// Weak reference wrapper backed by [`alloc::sync::Weak`] by default.
///
/// When the `force-portable-arc` feature is enabled it switches to [`portable_atomic_util::Weak`]
/// so that targets without native atomic pointer support still benefit from a `Weak`-compatible
/// shared handle.
#[repr(transparent)]
pub struct WeakShared<T: ?Sized>(Weak<T>);

impl<T: ?Sized> WeakShared<T> {
  /// Creates a new `WeakShared` that points to nothing.
  #[must_use]
  pub const fn new() -> Self
  where
    T: Sized, {
    Self(Weak::new())
  }

  /// Wraps an existing [`Weak`] inside the shared wrapper.
  #[must_use]
  pub const fn from_weak(inner: Weak<T>) -> Self {
    Self(inner)
  }

  /// Attempts to upgrade the weak reference to an [`ArcShared`].
  ///
  /// Returns `None` if the inner value has been dropped.
  #[must_use]
  pub fn upgrade(&self) -> Option<ArcShared<T>> {
    self.0.upgrade().map(ArcShared::from_arc)
  }

  /// Returns the number of strong references pointing to this allocation.
  ///
  /// Returns 0 if the value has been dropped.
  #[must_use]
  pub fn strong_count(&self) -> usize {
    self.0.strong_count()
  }

  /// Returns the number of weak references pointing to this allocation.
  #[must_use]
  pub fn weak_count(&self) -> usize {
    self.0.weak_count()
  }
}

impl<T> Default for WeakShared<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T: ?Sized> Clone for WeakShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T: ?Sized + core::fmt::Debug> core::fmt::Debug for WeakShared<T> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("WeakShared").finish()
  }
}
