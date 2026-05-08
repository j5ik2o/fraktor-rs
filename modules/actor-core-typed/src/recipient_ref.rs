//! Common recipient abstraction for typed and untyped actor references.

#[cfg(test)]
mod tests;

use fraktor_actor_core_rs::core::kernel::actor::{
  Pid,
  actor_ref::ActorRef,
  messaging::{AnyMessage, AskResponse},
};

use crate::{TypedActorRef, dsl::TypedAskResponse};

/// Common abstraction over references that can receive typed messages.
pub trait RecipientRef<M>: Send + Sync
where
  M: Send + Sync + 'static, {
  /// Reply reference type exposed by `ask`.
  type ReplyRef<R>: Send + Sync + 'static
  where
    R: Send + Sync + 'static;

  /// Ask response type produced by `ask`.
  type AskResponse<R>
  where
    R: Send + Sync + 'static;

  /// Returns the pid of the recipient.
  #[must_use]
  fn pid(&self) -> Pid;

  /// Delivers `message` to the recipient.
  #[cfg(not(fraktor_disable_tell))]
  fn tell(&mut self, message: M);

  /// Sends a typed request and obtains the ask response.
  ///
  /// # Errors
  ///
  /// Returns an error if the request cannot be sent.
  fn ask<R, F>(&mut self, build: F) -> Self::AskResponse<R>
  where
    R: Send + Sync + 'static,
    F: FnOnce(Self::ReplyRef<R>) -> M;
}

impl<M> RecipientRef<M> for TypedActorRef<M>
where
  M: Send + Sync + 'static,
{
  type AskResponse<R>
    = TypedAskResponse<R>
  where
    R: Send + Sync + 'static;
  type ReplyRef<R>
    = TypedActorRef<R>
  where
    R: Send + Sync + 'static;

  fn pid(&self) -> Pid {
    TypedActorRef::pid(self)
  }

  #[cfg(not(fraktor_disable_tell))]
  fn tell(&mut self, message: M) {
    TypedActorRef::tell(self, message)
  }

  fn ask<R, F>(&mut self, build: F) -> Self::AskResponse<R>
  where
    R: Send + Sync + 'static,
    F: FnOnce(Self::ReplyRef<R>) -> M, {
    TypedActorRef::ask(self, build)
  }
}

impl<M> RecipientRef<M> for ActorRef
where
  M: Send + Sync + 'static,
{
  type AskResponse<R>
    = AskResponse
  where
    R: Send + Sync + 'static;
  type ReplyRef<R>
    = ActorRef
  where
    R: Send + Sync + 'static;

  fn pid(&self) -> Pid {
    ActorRef::pid(self)
  }

  #[cfg(not(fraktor_disable_tell))]
  fn tell(&mut self, message: M) {
    ActorRef::tell(self, AnyMessage::new(message));
  }

  fn ask<R, F>(&mut self, build: F) -> Self::AskResponse<R>
  where
    R: Send + Sync + 'static,
    F: FnOnce(Self::ReplyRef<R>) -> M, {
    self.ask_with_factory(true, |reply_ref| AnyMessage::new(build(reply_ref)))
  }
}
