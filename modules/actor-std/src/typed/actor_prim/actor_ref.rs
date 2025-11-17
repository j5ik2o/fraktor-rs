use std::ops::{Deref, DerefMut};

use fraktor_actor_core_rs::typed::actor_prim::TypedActorRefGeneric;
use fraktor_utils_core_rs::std::runtime_toolbox::StdToolbox;

#[repr(transparent)]
/// Strongly typed actor reference bound to the standard runtime toolbox.
pub struct TypedActorRef<M>
where
  M: Send + Sync + 'static, {
  inner: TypedActorRefGeneric<M, StdToolbox>,
}

impl<M> TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  /// Wraps a core typed actor reference using the standard toolbox.
  pub const fn from_core(inner: TypedActorRefGeneric<M, StdToolbox>) -> Self {
    Self { inner }
  }

  /// Returns the underlying core reference as an immutable view.
  pub const fn as_core(&self) -> &TypedActorRefGeneric<M, StdToolbox> {
    &self.inner
  }

  /// Returns the underlying core reference as a mutable view.
  pub fn as_core_mut(&mut self) -> &mut TypedActorRefGeneric<M, StdToolbox> {
    &mut self.inner
  }

  /// Consumes the wrapper and exposes the core reference.
  pub fn into_core(self) -> TypedActorRefGeneric<M, StdToolbox> {
    self.inner
  }
}

impl<M> Deref for TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  type Target = TypedActorRefGeneric<M, StdToolbox>;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<M> DerefMut for TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}
