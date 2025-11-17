//! Typed props.

use core::marker::PhantomData;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{
  props::PropsGeneric,
  typed::{
    actor_prim::TypedActor, behavior::Behavior, behavior_runner::BehaviorRunner, typed_actor_adapter::TypedActorAdapter,
  },
};

/// Describes how to construct a typed actor for message `M`.
pub struct TypedPropsGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  props:  PropsGeneric<TB>,
  marker: PhantomData<M>,
}

/// Type alias for [TypedPropsGeneric] with the default [NoStdToolbox].
pub type TypedProps<M> = TypedPropsGeneric<M, NoStdToolbox>;

impl<M, TB> Clone for TypedPropsGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn clone(&self) -> Self {
    Self { props: self.props.clone(), marker: PhantomData }
  }
}

impl<M, TB> TypedPropsGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Builds props from a typed actor factory.
  #[must_use]
  pub fn new<F, A>(factory: F) -> Self
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: TypedActor<M, TB> + 'static, {
    let props = PropsGeneric::from_fn(move || TypedActorAdapter::<M, TB>::new(factory()));
    Self { props, marker: PhantomData }
  }

  /// Builds props from a typed behavior factory.
  #[must_use]
  pub fn from_behavior_factory<F>(factory: F) -> Self
  where
    F: Fn() -> Behavior<M, TB> + Send + Sync + 'static, {
    let props = PropsGeneric::from_fn(move || {
      let behavior = factory();
      TypedActorAdapter::<M, TB>::new(BehaviorRunner::new(behavior))
    });
    Self { props, marker: PhantomData }
  }

  /// Backwards-compatible alias for [`TypedPropsGeneric::new`].
  #[must_use]
  pub fn from_factory<F, A>(factory: F) -> Self
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: TypedActor<M, TB> + 'static, {
    Self::new(factory)
  }

  /// Wraps existing props after applying an external typed conversion.
  #[must_use]
  pub const fn from_props(props: PropsGeneric<TB>) -> Self {
    Self { props, marker: PhantomData }
  }

  /// Returns the underlying props.
  #[must_use]
  pub const fn to_untyped(&self) -> &PropsGeneric<TB> {
    &self.props
  }

  /// Consumes the typed props and returns the props.
  #[must_use]
  pub fn into_untyped(self) -> PropsGeneric<TB> {
    self.props
  }

  /// Applies a mapping function to the props and returns a new typed props.
  #[must_use]
  pub fn map_props(self, f: impl FnOnce(PropsGeneric<TB>) -> PropsGeneric<TB>) -> Self {
    Self { props: f(self.props), marker: PhantomData }
  }
}
