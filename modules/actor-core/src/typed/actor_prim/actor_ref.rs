//! Typed actor reference wrapper.

use core::marker::PhantomData;

use fraktor_utils_core_rs::sync::NoStdToolbox;

use crate::{
  RuntimeToolbox,
  actor_prim::{Pid, actor_ref::ActorRefGeneric},
  error::SendError,
  messaging::AnyMessageGeneric,
  typed::TypedAskResponseGeneric,
};

/// Provides a typed façade over [`ActorRefGeneric`].
pub struct TypedActorRefGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  inner:   ActorRefGeneric<TB>,
  _marker: PhantomData<M>,
}

/// Type alias for [TypedActorRefGeneric] with the default [NoStdToolbox].
pub type TypedActorRef<M> = TypedActorRefGeneric<M, NoStdToolbox>;

impl<M, TB> TypedActorRefGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Wraps an untyped actor reference.
  #[must_use]
  pub const fn from_untyped(inner: ActorRefGeneric<TB>) -> Self {
    Self { inner, _marker: PhantomData }
  }

  /// Returns the underlying untyped reference.
  #[must_use]
  pub const fn as_untyped(&self) -> &ActorRefGeneric<TB> {
    &self.inner
  }

  /// Consumes the wrapper and returns the untyped reference.
  #[must_use]
  pub fn into_untyped(self) -> ActorRefGeneric<TB> {
    self.inner
  }

  /// Returns the actor pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.inner.pid()
  }

  /// Sends a typed message to the actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be delivered.
  pub fn tell(&self, message: M) -> Result<(), SendError<TB>> {
    self.inner.tell(AnyMessageGeneric::new(message))
  }

  /// Sends a typed request and obtains the ask response.
  ///
  /// # Errors
  ///
  /// Returns an error if the request cannot be sent.
  pub fn ask<R>(&self, message: M) -> Result<TypedAskResponseGeneric<R, TB>, SendError<TB>>
  where
    R: Send + Sync + 'static, {
    let response = self.inner.ask(AnyMessageGeneric::new(message))?;
    Ok(TypedAskResponseGeneric::from_generic(response))
  }

  /// Maps this reference to a different message type without runtime cost.
  #[must_use]
  pub fn map<N>(self) -> TypedActorRefGeneric<N, TB>
  where
    N: Send + Sync + 'static, {
    TypedActorRefGeneric::from_untyped(self.inner)
  }
}

impl<M, TB> Clone for TypedActorRefGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
  }
}

impl<M, TB> core::fmt::Debug for TypedActorRefGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("TypedActorRefGeneric").field("pid", &self.pid()).finish()
  }
}
