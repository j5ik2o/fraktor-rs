//! Typed child reference wrapper.

use core::marker::PhantomData;

use crate::core::{
  actor::{ChildRef, Pid},
  error::SendError,
  messaging::AnyMessage,
  typed::{TypedAskResponse, actor::actor_ref::TypedActorRef},
};

/// Wraps [`ChildRef`] and enforces message type `M`.
pub struct TypedChildRef<M>
where
  M: Send + Sync + 'static, {
  inner:   ChildRef,
  _marker: PhantomData<M>,
}

impl<M> TypedChildRef<M>
where
  M: Send + Sync + 'static,
{
  /// Creates a typed wrapper from an untyped child reference.
  #[must_use]
  pub const fn from_untyped(inner: ChildRef) -> Self {
    Self { inner, _marker: PhantomData }
  }

  /// Returns the pid of the child actor.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.inner.pid()
  }

  /// Returns the typed actor reference for the child.
  #[must_use]
  pub fn actor_ref(&self) -> TypedActorRef<M> {
    TypedActorRef::from_untyped(self.inner.actor_ref().clone())
  }

  /// Sends a typed message to the child.
  #[cfg(not(fraktor_disable_tell))]
  pub fn tell(&mut self, message: M) {
    self.inner.tell(AnyMessage::new(message));
  }

  /// Sends a typed message to the child and preserves synchronous enqueue
  /// failures.
  ///
  /// # Errors
  ///
  /// Returns an error when the child mailbox rejects the message.
  pub fn try_tell(&mut self, message: M) -> Result<(), SendError> {
    self.inner.try_tell(AnyMessage::new(message))
  }

  /// Sends a typed request to the child actor.
  ///
  /// The request message is built with an explicit reply target.
  ///
  /// # Errors
  ///
  /// Returns an error if the request cannot be sent.
  pub fn ask<R, F>(&mut self, build: F) -> TypedAskResponse<R>
  where
    R: Send + Sync + 'static,
    F: FnOnce(TypedActorRef<R>) -> M, {
    let mut actor_ref = self.actor_ref();
    actor_ref.ask(build)
  }

  /// Stops the child actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop signal cannot be sent.
  pub fn stop(&self) -> Result<(), SendError> {
    self.inner.stop()
  }

  /// Suspends the child actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the suspend signal cannot be sent.
  pub fn suspend(&self) -> Result<(), SendError> {
    self.inner.suspend()
  }

  /// Resumes the child actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the resume signal cannot be sent.
  pub fn resume(&self) -> Result<(), SendError> {
    self.inner.resume()
  }

  /// Exposes the untyped handle when necessary.
  #[must_use]
  pub const fn as_untyped(&self) -> &ChildRef {
    &self.inner
  }

  /// Consumes the wrapper and returns the typed actor reference.
  #[must_use]
  pub fn into_actor_ref(self) -> TypedActorRef<M> {
    TypedActorRef::from_untyped(self.inner.into_actor_ref())
  }

  /// Consumes the wrapper and returns the untyped child reference.
  #[must_use]
  pub fn into_untyped(self) -> ChildRef {
    self.inner
  }
}

impl<M> Clone for TypedChildRef<M>
where
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
  }
}

impl<M> core::fmt::Debug for TypedChildRef<M>
where
  M: Send + Sync + 'static,
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("TypedChildRef").field("pid", &self.pid()).finish()
  }
}
