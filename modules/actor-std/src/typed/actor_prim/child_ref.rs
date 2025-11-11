use std::ops::{Deref, DerefMut};

use fraktor_actor_core_rs::typed::actor_prim::TypedChildRefGeneric;
use fraktor_utils_std_rs::runtime_toolbox::StdToolbox;

#[repr(transparent)]
/// Type-safe handle to a child actor running on the standard runtime.
pub struct TypedChildRef<M>
where
  M: Send + Sync + 'static, {
  inner: TypedChildRefGeneric<M, StdToolbox>,
}

impl<M> TypedChildRef<M>
where
  M: Send + Sync + 'static,
{
  /// Wraps a core child reference into the standard typed variant.
  pub const fn from_core(inner: TypedChildRefGeneric<M, StdToolbox>) -> Self {
    Self { inner }
  }

  /// Borrows the underlying core reference.
  pub const fn as_core(&self) -> &TypedChildRefGeneric<M, StdToolbox> {
    &self.inner
  }

  /// Mutably borrows the underlying core reference.
  pub fn as_core_mut(&mut self) -> &mut TypedChildRefGeneric<M, StdToolbox> {
    &mut self.inner
  }

  /// Consumes the wrapper and returns the core reference.
  pub fn into_core(self) -> TypedChildRefGeneric<M, StdToolbox> {
    self.inner
  }
}

impl<M> Deref for TypedChildRef<M>
where
  M: Send + Sync + 'static,
{
  type Target = TypedChildRefGeneric<M, StdToolbox>;

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
