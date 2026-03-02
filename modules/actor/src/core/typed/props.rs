//! Typed props.

use core::marker::PhantomData;

use crate::core::{
  props::Props,
  typed::{
    actor::TypedActor, behavior::Behavior, behavior_runner::BehaviorRunner, typed_actor_adapter::TypedActorAdapter,
  },
};

/// Describes how to construct a typed actor for message `M`.
pub struct TypedProps<M>
where
  M: Send + Sync + 'static, {
  props:  Props,
  marker: PhantomData<M>,
}

impl<M> Clone for TypedProps<M>
where
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self { props: self.props.clone(), marker: PhantomData }
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
    let props = Props::from_fn(move || TypedActorAdapter::<M>::new(factory()));
    Self { props, marker: PhantomData }
  }

  /// Builds props from a typed behavior factory.
  #[must_use]
  pub fn from_behavior_factory<F>(factory: F) -> Self
  where
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    let props = Props::from_fn(move || {
      let behavior = factory();
      TypedActorAdapter::<M>::new(BehaviorRunner::new(behavior))
    });
    Self { props, marker: PhantomData }
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
  pub const fn from_props(props: Props) -> Self {
    Self { props, marker: PhantomData }
  }

  /// Returns the underlying props.
  #[must_use]
  pub const fn to_untyped(&self) -> &Props {
    &self.props
  }

  /// Consumes the typed props and returns the props.
  #[must_use]
  pub fn into_untyped(self) -> Props {
    self.props
  }

  /// Applies a mapping function to the props and returns a new typed props.
  #[must_use]
  pub fn map_props(self, f: impl FnOnce(Props) -> Props) -> Self {
    Self { props: f(self.props), marker: PhantomData }
  }
}
