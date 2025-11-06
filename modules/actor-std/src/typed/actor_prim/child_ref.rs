use cellactor_actor_core_rs::typed::actor_prim::TypedChildRefGeneric;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;
use std::ops::{Deref, DerefMut};

#[repr(transparent)]
pub struct TypedChildRef<M>
where
  M: Send + Sync + 'static,
{
  inner: TypedChildRefGeneric<M, StdToolbox>,
}

impl<M> TypedChildRef<M>
where
  M: Send + Sync + 'static,
{
  pub const fn from_core(inner: TypedChildRefGeneric<M, StdToolbox>) -> Self {
    Self { inner }
  }

  pub const fn as_core(&self) -> &TypedChildRefGeneric<M, StdToolbox> {
    &self.inner
  }

  pub fn as_core_mut(&mut self) -> &mut TypedChildRefGeneric<M, StdToolbox> {
    &mut self.inner
  }

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
