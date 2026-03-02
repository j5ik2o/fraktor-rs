extern crate std;

use std::ops::{Deref, DerefMut};

use crate::core::typed::actor::TypedActorRef as CoreTypedActorRef;

#[repr(transparent)]
/// Strongly typed actor reference bound to the standard runtime toolbox.
pub struct TypedActorRef<M>
where
  M: Send + Sync + 'static, {
  inner: CoreTypedActorRef<M>,
}

impl<M> TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  /// Wraps a core typed actor reference using the standard toolbox.
  #[must_use]
  pub const fn from_core(inner: CoreTypedActorRef<M>) -> Self {
    Self { inner }
  }

  /// Returns the underlying core reference as an immutable view.
  #[must_use]
  pub const fn as_core(&self) -> &CoreTypedActorRef<M> {
    &self.inner
  }

  /// Returns the underlying core reference as a mutable view.
  pub const fn as_core_mut(&mut self) -> &mut CoreTypedActorRef<M> {
    &mut self.inner
  }

  /// Consumes the wrapper and exposes the core reference.
  #[must_use]
  pub fn into_core(self) -> CoreTypedActorRef<M> {
    self.inner
  }
}

impl<M> Deref for TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  type Target = CoreTypedActorRef<M>;

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

impl<M> Clone for TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
