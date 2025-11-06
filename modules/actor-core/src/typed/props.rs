//! Typed behavior builder.

use core::marker::PhantomData;

use cellactor_utils_core_rs::sync::NoStdToolbox;

use crate::{
  RuntimeToolbox,
  props::PropsGeneric,
  typed::{actor_prim::TypedActor, behavior_adapter::TypedActorAdapter},
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
  /// Builds behavior from a typed actor factory.
  #[must_use]
  pub fn new<F, A>(factory: F) -> Self
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: TypedActor<M, TB> + 'static, {
    let props = PropsGeneric::from_fn(move || TypedActorAdapter::<TB, M>::new(factory()));
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
  pub const fn props(&self) -> &PropsGeneric<TB> {
    &self.props
  }

  /// Consumes the behavior and returns the props.
  #[must_use]
  pub fn into_props(self) -> PropsGeneric<TB> {
    self.props
  }

  /// Applies a mapping function to the props and returns a new behavior.
  #[must_use]
  pub fn map_props(self, f: impl FnOnce(PropsGeneric<TB>) -> PropsGeneric<TB>) -> Self {
    Self { props: f(self.props), marker: PhantomData }
  }
}
