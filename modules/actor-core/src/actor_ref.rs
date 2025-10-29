//! Actor handle abstraction used by message envelopes and contexts.

use core::fmt;

use cellactor_utils_core_rs::ArcShared;

use crate::{
  actor_future::ActorFuture, actor_ref_backend::ActorRefBackend, any_message::AnyOwnedMessage,
  ask_reply_endpoint::AskReplyEndpoint, pid::Pid, send_error::SendError,
};

/// Lightweight handle pointing at an actor cell or temporary reply endpoint.
#[derive(Clone)]
pub struct ActorRef {
  pid:     Option<Pid>,
  backend: Option<ArcShared<dyn ActorRefBackend>>,
}

impl ActorRef {
  /// Creates a dangling handle that is not associated with any PID.
  #[must_use]
  pub const fn dangling() -> Self {
    Self { pid: None, backend: None }
  }

  /// Creates a handle backed by the provided PID and backend.
  #[must_use]
  pub fn new(pid: Pid, backend: ArcShared<dyn ActorRefBackend>) -> Self {
    Self { pid: Some(pid), backend: Some(backend) }
  }

  /// Internal constructor for virtual references such as ask reply endpoints.
  pub(crate) fn with_backend(pid: Option<Pid>, backend: ArcShared<dyn ActorRefBackend>) -> Self {
    Self { pid, backend: Some(backend) }
  }

  /// Returns the PID associated with the handle if it is known.
  #[must_use]
  pub const fn pid(&self) -> Option<&Pid> {
    self.pid.as_ref()
  }

  /// Returns `true` when the reference does not point to a live actor.
  #[must_use]
  pub const fn is_dangling(&self) -> bool {
    self.backend.is_none()
  }

  /// Sends a message to the referenced actor.
  pub fn tell(&self, message: AnyOwnedMessage) -> Result<(), SendError<AnyOwnedMessage>> {
    let Some(backend) = self.backend.as_ref() else {
      return Err(SendError::no_recipient(self.pid, message));
    };
    backend.send(message)
  }

  /// Sends a message and returns a future that resolves when a reply arrives.
  pub fn ask(
    &self,
    message: AnyOwnedMessage,
  ) -> Result<ArcShared<ActorFuture<AnyOwnedMessage>>, SendError<AnyOwnedMessage>> {
    let Some(backend) = self.backend.as_ref() else {
      return Err(SendError::no_recipient(self.pid, message));
    };
    let future = ArcShared::new(ActorFuture::<AnyOwnedMessage>::new());
    let reply_backend: ArcShared<AskReplyEndpoint> = ArcShared::new(AskReplyEndpoint::new(future.clone()));
    let reply_backend: ArcShared<dyn ActorRefBackend> = reply_backend;
    let reply_to = ActorRef::with_backend(None, reply_backend);
    let enriched = message.with_reply_to(reply_to);
    backend.send(enriched)?;
    Ok(future)
  }
}

impl fmt::Debug for ActorRef {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut builder = f.debug_struct("ActorRef");
    match &self.pid {
      | Some(pid) => {
        builder.field("pid", pid);
      },
      | None => {
        builder.field("pid", &"unbound");
      },
    }
    builder.field("dangling", &self.is_dangling()).finish()
  }
}
