//! Typed actor reference wrapper.

use core::marker::PhantomData;

use fraktor_utils_rs::core::sync::SharedAccess;

use crate::core::{
  kernel::{
    actor::{
      Pid,
      actor_path::ActorPath,
      actor_ref::{ActorRef, AskReplySender},
      messaging::{AnyMessage, AskError, AskResponse, AskResult},
    },
    util::futures::ActorFutureShared,
  },
  typed::dsl::{StatusReply, TypedAskResponse},
};

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
  pub fn try_tell(&mut self, message: M) -> Result<(), crate::core::kernel::actor::error::SendError> {
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
    let future = ActorFutureShared::<AskResult>::new();
    let reply_sender = AskReplySender::new(future.clone());
    let system = self.inner.system_state();
    let reply_ref = if let Some(system) = &system {
      ActorRef::with_system(self.inner.pid(), reply_sender, system)
    } else {
      ActorRef::new(self.inner.pid(), reply_sender)
    };
    let reply_typed = TypedActorRef::from_untyped(reply_ref.clone());
    let message = build(reply_typed);
    if let Err(error) = self.inner.try_tell(AnyMessage::new(message)) {
      let waker = future.with_write(|inner| inner.complete(Err(AskError::from(&error))));
      if let Some(waker) = waker {
        waker.wake();
      }
    } else if let Some(system) = system {
      system.register_ask_future(future.clone());
    }
    TypedAskResponse::from_generic(AskResponse::new(reply_ref, future))
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
    TypedActorRef::from_untyped(self.inner)
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
    TypedActorRef::from_untyped(self.inner)
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

impl<M> core::fmt::Debug for TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("TypedActorRef").field("pid", &self.pid()).finish()
  }
}
