//! Common recipient abstraction for typed and untyped actor references.

#[cfg(test)]
mod tests;

use crate::core::{
  actor::{
    Pid,
    actor_ref::{ActorRef, AskReplySender},
  },
  error::SendError,
  futures::ActorFutureShared,
  messaging::{AnyMessage, AskResponse, AskResult},
  typed::{TypedAskResponse, actor::TypedActorRef},
};

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
  ///
  /// # Errors
  ///
  /// Returns an error if the message cannot be enqueued.
  fn tell(&mut self, message: M) -> Result<(), SendError>;

  /// Sends a typed request and obtains the ask response.
  ///
  /// # Errors
  ///
  /// Returns an error if the request cannot be sent.
  fn ask<R, F>(&mut self, build: F) -> Result<Self::AskResponse<R>, SendError>
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

  fn tell(&mut self, message: M) -> Result<(), SendError> {
    TypedActorRef::tell(self, message)
  }

  fn ask<R, F>(&mut self, build: F) -> Result<Self::AskResponse<R>, SendError>
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

  fn tell(&mut self, message: M) -> Result<(), SendError> {
    ActorRef::tell(self, AnyMessage::new(message))
  }

  fn ask<R, F>(&mut self, build: F) -> Result<Self::AskResponse<R>, SendError>
  where
    R: Send + Sync + 'static,
    F: FnOnce(Self::ReplyRef<R>) -> M, {
    let future = ActorFutureShared::<AskResult>::new();
    let reply_sender = AskReplySender::new(future.clone());
    let reply_ref = if let Some(system) = self.system_state() {
      let reply_ref = ActorRef::with_system(self.pid(), reply_sender, &system);
      system.register_ask_future(future.clone());
      reply_ref
    } else {
      ActorRef::new(self.pid(), reply_sender)
    };
    let message = build(reply_ref.clone());
    ActorRef::tell(self, AnyMessage::new(message))?;
    Ok(AskResponse::new(reply_ref, future))
  }
}
