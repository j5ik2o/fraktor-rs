use crate::{
  core::typed::{Behavior, TypedProps as CoreTypedProps},
  std::{
    props::Props,
    typed::actor::{TypedActor, TypedActorAdapter},
  },
};

/// Builder for typed actors and behaviors running on the standard runtime toolbox.
pub struct TypedProps<M>
where
  M: Send + Sync + 'static, {
  inner: CoreTypedProps<M>,
}

impl<M> Clone for TypedProps<M>
where
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<M> TypedProps<M>
where
  M: Send + Sync + 'static,
{
  /// Builds props from a typed actor factory.
  #[must_use]
  pub fn new<F, A>(factory: F) -> Self
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: TypedActor<M> + 'static, {
    let inner = CoreTypedProps::new(move || TypedActorAdapter::new(factory()));
    Self { inner }
  }

  /// Builds props from a typed behavior factory.
  #[must_use]
  pub fn from_behavior_factory<F>(factory: F) -> Self
  where
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    let inner = CoreTypedProps::from_behavior_factory(factory);
    Self { inner }
  }

  /// Backwards-compatible alias for [`TypedProps::new`].
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
    let inner = CoreTypedProps::from_props(props.into_inner());
    Self { inner }
  }

  /// Adopts already constructed core typed props that use the standard toolbox.
  #[must_use]
  pub const fn from_core(props: CoreTypedProps<M>) -> Self {
    Self { inner: props }
  }

  /// Returns the underlying core representation for advanced configuration.
  #[must_use]
  pub const fn as_core(&self) -> &CoreTypedProps<M> {
    &self.inner
  }

  /// Consumes the wrapper and yields the core props value.
  #[must_use]
  pub fn into_core(self) -> CoreTypedProps<M> {
    self.inner
  }
}
