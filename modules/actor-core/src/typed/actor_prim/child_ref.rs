//! Typed child reference wrapper.

use core::marker::PhantomData;

use cellactor_utils_core_rs::sync::NoStdToolbox;

use crate::{
  RuntimeToolbox,
  actor_prim::{ChildRefGeneric, Pid},
  error::SendError,
  messaging::AnyMessageGeneric,
  typed::{TypedAskResponseGeneric, actor_prim::actor_ref::TypedActorRefGeneric},
};

/// Wraps [`ChildRefGeneric`] and enforces message type `M`.
pub struct TypedChildRefGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  inner:   ChildRefGeneric<TB>,
  _marker: PhantomData<M>,
}

/// Type alias for [TypedChildRefGeneric] with the default [NoStdToolbox].
pub type TypedChildRef<M> = TypedChildRefGeneric<M, NoStdToolbox>;

impl<M, TB> TypedChildRefGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a typed wrapper from an untyped child reference.
  #[must_use]
  pub const fn from_untyped(inner: ChildRefGeneric<TB>) -> Self {
    Self { inner, _marker: PhantomData }
  }

  /// Returns the pid of the child actor.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.inner.pid()
  }

  /// Returns the typed actor reference for the child.
  #[must_use]
  pub fn actor_ref(&self) -> TypedActorRefGeneric<M, TB> {
    TypedActorRefGeneric::from_untyped(self.inner.actor_ref().clone())
  }

  /// Sends a typed message to the child.
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be delivered.
  pub fn tell(&self, message: M) -> Result<(), SendError<TB>> {
    self.inner.tell(AnyMessageGeneric::new(message))
  }

  /// Sends a typed request to the child actor.
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

  /// Stops the child actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop signal cannot be sent.
  pub fn stop(&self) -> Result<(), SendError<TB>> {
    self.inner.stop()
  }

  /// Suspends the child actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the suspend signal cannot be sent.
  pub fn suspend(&self) -> Result<(), SendError<TB>> {
    self.inner.suspend()
  }

  /// Resumes the child actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the resume signal cannot be sent.
  pub fn resume(&self) -> Result<(), SendError<TB>> {
    self.inner.resume()
  }

  /// Exposes the untyped handle when necessary.
  #[must_use]
  pub const fn as_untyped(&self) -> &ChildRefGeneric<TB> {
    &self.inner
  }

  /// Consumes the wrapper and returns the untyped child reference.
  #[must_use]
  pub fn into_untyped(self) -> ChildRefGeneric<TB> {
    self.inner
  }
}

impl<M, TB> Clone for TypedChildRefGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
  }
}

impl<M, TB> core::fmt::Debug for TypedChildRefGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("TypedChildRefGeneric").field("pid", &self.pid()).finish()
  }
}
