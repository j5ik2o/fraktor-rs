//! Typed actor reference wrapper.

use core::marker::PhantomData;

use crate::core::{
  actor::{
    Pid,
    actor_ref::{ActorRef, AskReplySender},
  },
  error::SendError,
  futures::ActorFutureShared,
  messaging::{AnyMessage, AskResponse, AskResult},
  typed::TypedAskResponse,
};

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
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be delivered.
  pub fn tell(&mut self, message: M) -> Result<(), SendError> {
    self.inner.tell(AnyMessage::new(message))
  }

  /// Sends a typed request and obtains the ask response.
  ///
  /// The request message is built with an explicit reply target.
  /// The future resolves with `Ok(message)` on success, or `Err(AskError)` on failure.
  ///
  /// # Errors
  ///
  /// Returns an error if the request cannot be sent.
  pub fn ask<R, F>(&mut self, build: F) -> Result<TypedAskResponse<R>, SendError>
  where
    R: Send + Sync + 'static,
    F: FnOnce(TypedActorRef<R>) -> M, {
    let future = ActorFutureShared::<AskResult>::new();
    let reply_sender = AskReplySender::new(future.clone());
    let reply_ref = if let Some(system) = self.inner.system_state() {
      let reply_ref = ActorRef::with_system(self.inner.pid(), reply_sender, &system);
      system.register_ask_future(future.clone());
      reply_ref
    } else {
      ActorRef::new(self.inner.pid(), reply_sender)
    };
    let reply_typed = TypedActorRef::from_untyped(reply_ref.clone());
    let message = build(reply_typed);
    self.inner.tell(AnyMessage::new(message))?;
    Ok(TypedAskResponse::from_generic(AskResponse::new(reply_ref, future)))
  }

  /// Maps this reference to a different message type without runtime cost.
  #[must_use]
  pub fn map<N>(self) -> TypedActorRef<N>
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
