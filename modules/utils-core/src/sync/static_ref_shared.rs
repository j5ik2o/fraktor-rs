use core::ops::Deref;

use super::{Shared, SharedDyn};

#[cfg(test)]
mod tests;

/// Shared wrapper backed by a `'static` reference.
///
/// Thin wrapper that exposes user-supplied `'static` values through the [`Shared`] abstraction.
pub struct StaticRefShared<T: ?Sized + 'static>(&'static T);

impl<T: ?Sized + 'static> StaticRefShared<T> {
  /// Creates a new wrapper from a `'static` reference.
  #[must_use]
  pub const fn new(reference: &'static T) -> Self {
    Self(reference)
  }

  /// Returns the raw `'static` reference.
  #[must_use]
  pub const fn as_ref(self) -> &'static T {
    self.0
  }
}

impl<T: ?Sized + 'static> Deref for StaticRefShared<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    self.0
  }
}

impl<T: ?Sized + 'static> Clone for StaticRefShared<T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: ?Sized + 'static> Copy for StaticRefShared<T> {}

impl<T: ?Sized + 'static> Shared<T> for StaticRefShared<T> {}

impl<T: ?Sized + 'static> SharedDyn<T> for StaticRefShared<T> {
  type Dyn<U: ?Sized + 'static> = StaticRefShared<U>;

  fn into_dyn<U: ?Sized + 'static, F>(self, cast: F) -> Self::Dyn<U>
  where
    F: FnOnce(&T) -> &U, {
    let reference = cast(self.0);
    StaticRefShared::new(reference)
  }
}
