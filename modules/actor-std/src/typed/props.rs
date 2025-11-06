use crate::props::Props;
use crate::typed::actor_prim::{TypedActor, TypedActorAdapter};
use crate::typed::Behavior;
use cellactor_actor_core_rs::typed::TypedPropsGeneric as CoreTypedPropsGeneric;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

pub struct TypedProps<M> where M: Send + Sync + 'static {
  inner: CoreTypedPropsGeneric<M, StdToolbox>,
}

impl<M> Clone for TypedProps<M>
where
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<M> TypedProps<M> where M: Send + Sync + 'static {
  /// Builds props from a typed actor factory.
  #[must_use]
  pub fn new<F, A>(factory: F) -> Self
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: TypedActor<M> + 'static, {
    let inner = CoreTypedPropsGeneric::new(move || TypedActorAdapter::new(factory()));
    Self { inner }
  }

  /// Builds props from a typed behavior factory.
  #[must_use]
  pub fn from_behavior_factory<F>(factory: F) -> Self
  where
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    let inner = CoreTypedPropsGeneric::from_behavior_factory(factory);
    Self { inner }
  }

  /// Backwards-compatible alias for [`TypedPropsGeneric::new`].
  #[must_use]
  pub fn from_factory<F, A>(factory: F) -> Self
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: TypedActor<M> + 'static, {
    Self::new(factory)
  }

  /// Wraps existing props after applying an external typed conversion.
  #[must_use]
  pub fn from_props(props: Props) -> Self {
    let inner = CoreTypedPropsGeneric::from_props(props.into_inner());
    Self { inner }
  }


  pub fn from_core(props: CoreTypedPropsGeneric<M, StdToolbox>) -> Self {
    Self { inner: props }
  }

  pub fn as_core(&self) -> &CoreTypedPropsGeneric<M, StdToolbox> {
    &self.inner
  }

  pub fn into_core(self) -> CoreTypedPropsGeneric<M, StdToolbox> {
    self.inner
  }

}
