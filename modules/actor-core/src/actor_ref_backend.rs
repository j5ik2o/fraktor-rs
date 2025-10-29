//! Backend trait used by [`ActorRef`](crate::ActorRef) to enqueue messages.

use crate::{any_message::AnyOwnedMessage, pid::Pid, send_error::SendError};

/// Abstraction representing a message enqueue endpoint.
pub trait ActorRefBackend: Send + Sync {
  /// Returns the PID associated with this backend when known.
  fn pid(&self) -> Option<Pid> {
    None
  }

  /// Enqueues the provided message.
  ///
  /// The backend may apply backpressure or reject the message depending on mailbox policy.
  fn send(&self, message: AnyOwnedMessage) -> Result<(), SendError>;
}
