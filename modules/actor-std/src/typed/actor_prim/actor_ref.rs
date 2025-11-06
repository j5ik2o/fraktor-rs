use cellactor_actor_core_rs::typed::actor_prim::TypedActorRefGeneric;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;
use std::ops::{Deref, DerefMut};

#[repr(transparent)]
pub struct TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  inner: TypedActorRefGeneric<M, StdToolbox>,
}

impl<M> TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  pub const fn from_core(inner: TypedActorRefGeneric<M, StdToolbox>) -> Self {
    Self { inner }
  }

  pub const fn as_core(&self) -> &TypedActorRefGeneric<M, StdToolbox> {
    &self.inner
  }

  pub fn as_core_mut(&mut self) -> &mut TypedActorRefGeneric<M, StdToolbox> {
    &mut self.inner
  }

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
