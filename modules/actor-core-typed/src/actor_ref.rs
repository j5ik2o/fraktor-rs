//! Typed actor reference wrapper.

use core::{
  cmp::Ordering,
  fmt::{Debug, Formatter, Result as FmtResult},
  hash::{Hash, Hasher},
  marker::PhantomData,
};

use fraktor_actor_core_rs::core::kernel::actor::{
  Pid, actor_path::ActorPath, actor_ref::ActorRef, error::SendError, messaging::AnyMessage,
};

use crate::dsl::{StatusReply, TypedAskResponse};

#[cfg(test)]
mod tests;

/// Provides a typed facade over [`ActorRef`].
pub struct TypedActorRef<M>
where
  M: Send + Sync + 'static, {
  inner:   ActorRef,
  _marker: PhantomData<M>,
}

impl<M> TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  /// Wraps an untyped actor reference.
  #[must_use]
  pub const fn from_untyped(inner: ActorRef) -> Self {
    Self { inner, _marker: PhantomData }
  }

  /// Returns the underlying untyped reference.
  #[must_use]
  pub const fn as_untyped(&self) -> &ActorRef {
    &self.inner
  }

  /// Returns the underlying untyped reference mutably.
  #[must_use]
  pub const fn as_untyped_mut(&mut self) -> &mut ActorRef {
    &mut self.inner
  }

  /// Consumes the wrapper and returns the untyped reference.
  #[must_use]
  pub fn into_untyped(self) -> ActorRef {
    self.inner
  }

  /// Returns the actor pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.inner.pid()
  }

  /// Sends a typed message to the actor.
  #[cfg(not(fraktor_disable_tell))]
  pub fn tell(&mut self, message: M) {
    self.inner.tell(AnyMessage::new(message));
  }

  /// Sends a typed message to the actor and preserves synchronous enqueue
  /// failures.
  ///
  /// # Errors
  ///
  /// Returns an error when the underlying mailbox rejects the message.
  pub fn try_tell(&mut self, message: M) -> Result<(), SendError> {
    self.inner.try_tell(AnyMessage::new(message))
  }

  /// Sends a typed request and obtains the ask response.
  ///
  /// The request message is built with an explicit reply target.
  /// The future resolves with `Ok(message)` on success, or `Err(AskError)` on failure.
  ///
  /// # Errors
  ///
  /// Returns an error if the request cannot be sent.
  pub fn ask<R, F>(&mut self, build: F) -> TypedAskResponse<R>
  where
    R: Send + Sync + 'static,
    F: FnOnce(TypedActorRef<R>) -> M, {
    let response = self.inner.ask_with_factory(true, |reply_ref| {
      let reply_typed = TypedActorRef::from_untyped(reply_ref);
      AnyMessage::new(build(reply_typed))
    });
    TypedAskResponse::from_generic(response)
  }

  /// Sends a typed request expecting a [`StatusReply<R>`] response.
  ///
  /// This is a convenience wrapper around [`ask`](Self::ask) that constrains the
  /// reply type to [`StatusReply<R>`]. The returned [`StatusReply`] can be converted
  /// to `Result<R, StatusReplyError>` via [`into_result()`](StatusReply::into_result).
  ///
  /// # Errors
  ///
  /// Returns an error if the request cannot be sent.
  pub fn ask_with_status<R, F>(&mut self, build: F) -> TypedAskResponse<StatusReply<R>>
  where
    R: Send + Sync + 'static,
    F: FnOnce(TypedActorRef<StatusReply<R>>) -> M, {
    self.ask(build)
  }

  /// Maps this reference to a different message type without runtime cost.
  #[must_use]
  pub fn map<N>(self) -> TypedActorRef<N>
  where
    N: Send + Sync + 'static, {
    TypedActorRef::from_untyped(self.inner)
  }

  /// Returns the logical path of the actor if the system is still available.
  ///
  /// Corresponds to Pekko's `ActorRef.path`.
  #[must_use]
  pub fn path(&self) -> Option<ActorPath> {
    self.inner.path()
  }

  /// Narrows this reference to accept a subtype of messages.
  ///
  /// In Rust this is a type-level cast via `PhantomData` since Rust lacks
  /// subtyping on user types. Corresponds to Pekko's `ActorRef.narrow`.
  #[must_use]
  pub fn narrow<N>(self) -> TypedActorRef<N>
  where
    N: Send + Sync + 'static, {
    self.map()
  }

  /// Widens this reference to a supertype.
  ///
  /// # Safety (logical)
  ///
  /// The caller must ensure that the target actor can handle all messages
  /// of type `N`. Corresponds to Pekko's `ActorRef.unsafeUpcast`.
  #[must_use]
  pub fn unsafe_upcast<N>(self) -> TypedActorRef<N>
  where
    N: Send + Sync + 'static, {
    self.map()
  }
}

impl<M> Clone for TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
  }
}

impl<M> Debug for TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("TypedActorRef").field("pid", &self.pid()).finish()
  }
}

impl<M> PartialEq for TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  fn eq(&self, other: &Self) -> bool {
    self.pid() == other.pid()
  }
}

impl<M> Eq for TypedActorRef<M> where M: Send + Sync + 'static {}

impl<M> Hash for TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.pid().hash(state);
  }
}

impl<M> PartialOrd for TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<M> Ord for TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  fn cmp(&self, other: &Self) -> Ordering {
    (self.pid().value(), self.pid().generation()).cmp(&(other.pid().value(), other.pid().generation()))
  }
}
