extern crate std;
use std::ops::{Deref, DerefMut};

use crate::core::typed::actor::TypedChildRef as CoreTypedChildRef;

#[repr(transparent)]
/// Type-safe handle to a child actor running on the standard runtime.
pub struct TypedChildRef<M>
where
  M: Send + Sync + 'static, {
  inner: CoreTypedChildRef<M>,
}

impl<M> TypedChildRef<M>
where
  M: Send + Sync + 'static,
{
  /// Wraps a core child reference into the standard typed variant.
  #[must_use]
  pub const fn from_core(inner: CoreTypedChildRef<M>) -> Self {
    Self { inner }
  }

  /// Borrows the underlying core reference.
  #[must_use]
  pub const fn as_core(&self) -> &CoreTypedChildRef<M> {
    &self.inner
  }

  /// Mutably borrows the underlying core reference.
  pub const fn as_core_mut(&mut self) -> &mut CoreTypedChildRef<M> {
    &mut self.inner
  }

  /// Consumes the wrapper and returns the core reference.
  #[must_use]
  pub fn into_core(self) -> CoreTypedChildRef<M> {
    self.inner
  }
}

impl<M> Deref for TypedChildRef<M>
where
  M: Send + Sync + 'static,
{
  type Target = CoreTypedChildRef<M>;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<M> DerefMut for TypedChildRef<M>
where
  M: Send + Sync + 'static,
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}
