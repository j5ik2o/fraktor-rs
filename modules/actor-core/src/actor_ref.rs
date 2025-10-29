//! ActorRef handle used for message dispatching.

use core::fmt;

use crate::{
  actor_future::ActorFuture,
  any_owned_message::AnyOwnedMessage,
  pid::Pid,
  send_error::SendError,
  system::{SystemShared, enqueue_system, enqueue_user},
};

/// Reference to a running actor managed by [`ActorSystem`](crate::system::ActorSystem).
#[derive(Clone)]
pub struct ActorRef {
  kind: ActorRefKind,
}

#[derive(Clone)]
enum ActorRefKind {
  Runtime { pid: Pid, system: SystemShared },
  Ask { future: ActorFuture<AnyOwnedMessage> },
}

impl ActorRef {
  /// Creates a new handle backed by the shared system state.
  pub(crate) fn new(pid: Pid, system: SystemShared) -> Self {
    Self { kind: ActorRefKind::Runtime { pid, system } }
  }

  pub(crate) fn for_future(future: ActorFuture<AnyOwnedMessage>) -> Self {
    Self { kind: ActorRefKind::Ask { future } }
  }

  /// Returns the referenced PID.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    match &self.kind {
      | ActorRefKind::Runtime { pid, .. } => *pid,
      | ActorRefKind::Ask { .. } => Pid::new(0, 0),
    }
  }

  /// Sends a user message to the actor.
  pub fn tell(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    match &self.kind {
      | ActorRefKind::Runtime { pid, system } => enqueue_user(system, *pid, message),
      | ActorRefKind::Ask { future } => {
        future.complete(message);
        Ok(())
      },
    }
  }

  /// Sends a system message to the actor.
  pub fn tell_system(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    match &self.kind {
      | ActorRefKind::Runtime { pid, system } => enqueue_system(system, *pid, message),
      | ActorRefKind::Ask { .. } => Err(SendError::UnknownPid),
    }
  }

  /// Sends a request expecting a reply. The runtime attaches an internal reply handle
  /// so that responders can call `reply_to.tell(...)` and complete the returned future.
  pub fn ask(&self, message: AnyOwnedMessage) -> Result<ActorFuture<AnyOwnedMessage>, SendError> {
    match &self.kind {
      | ActorRefKind::Runtime { pid, system } => {
        let future = ActorFuture::pending();
        let reply_to = future.reply_handle();
        let message = message.with_reply_to(reply_to);
        enqueue_user(system, *pid, message)?;
        Ok(future)
      },
      | ActorRefKind::Ask { .. } => Err(SendError::UnknownPid),
    }
  }
}

impl PartialEq for ActorRef {
  fn eq(&self, other: &Self) -> bool {
    match (&self.kind, &other.kind) {
      | (ActorRefKind::Runtime { pid: left, .. }, ActorRefKind::Runtime { pid: right, .. }) => left == right,
      | _ => false,
    }
  }
}

impl Eq for ActorRef {}

impl fmt::Debug for ActorRef {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &self.kind {
      | ActorRefKind::Runtime { pid, .. } => f.debug_tuple("ActorRef").field(pid).finish(),
      | ActorRefKind::Ask { .. } => f.write_str("ActorRef(ask)"),
    }
  }
}
