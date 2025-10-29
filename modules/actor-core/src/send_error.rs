//! Errors that may occur when enqueuing messages via [`ActorRef`].

use crate::actor_error::ActorError;

/// Indicates why a message could not be enqueued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
  /// Mailbox is full and rejected the user message.
  MailboxFull,
  /// Mailbox is suspended for user traffic.
  MailboxSuspended,
  /// Mailbox policy configured to drop the newest message.
  DroppedNewest,
  /// Target PID was unknown (actor stopped or not yet registered).
  UnknownPid,
  /// Actor failed to start or process the message.
  ActorFailure(ActorError),
}

impl From<ActorError> for SendError {
  fn from(err: ActorError) -> Self {
    SendError::ActorFailure(err)
  }
}
