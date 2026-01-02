//! Typed actor reference wrapper.

use core::marker::PhantomData;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{
  actor::{
    Pid,
    actor_ref::{ActorRefGeneric, AskReplySenderGeneric},
  },
  error::SendError,
  futures::ActorFutureSharedGeneric,
  messaging::{AnyMessageGeneric, AskResponseGeneric, AskResult},
  typed::TypedAskResponseGeneric,
};

/// Provides a typed facade over [`ActorRefGeneric`].
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
  pub fn tell(&mut self, message: M) -> Result<(), SendError<TB>> {
    self.inner.tell(AnyMessageGeneric::new(message))
  }

  /// Sends a typed request and obtains the ask response.
  ///
  /// The request message is built with an explicit reply target.
  /// The future resolves with `Ok(message)` on success, or `Err(AskError)` on failure.
  ///
  /// # Errors
  ///
  /// Returns an error if the request cannot be sent.
  pub fn ask<R, F>(&mut self, build: F) -> Result<TypedAskResponseGeneric<R, TB>, SendError<TB>>
  where
    R: Send + Sync + 'static,
    F: FnOnce(TypedActorRefGeneric<R, TB>) -> M, {
    let future = ActorFutureSharedGeneric::<AskResult<TB>, TB>::new();
    let reply_sender = AskReplySenderGeneric::<TB>::new(future.clone());
    let reply_ref = if let Some(system) = self.inner.system_state() {
      let reply_ref = ActorRefGeneric::with_system(self.inner.pid(), reply_sender, &system);
      system.register_ask_future(future.clone());
      reply_ref
    } else {
      ActorRefGeneric::new(self.inner.pid(), reply_sender)
    };
    let reply_typed = TypedActorRefGeneric::from_untyped(reply_ref.clone());
    let message = build(reply_typed);
    self.inner.tell(AnyMessageGeneric::new(message))?;
    Ok(TypedAskResponseGeneric::from_generic(AskResponseGeneric::new(reply_ref, future)))
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
