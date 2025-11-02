//! Abstraction for delivering messages drained from the mailbox to actors.

use crate::{actor_error::ActorError, any_message::AnyMessage, RuntimeToolbox, SystemMessage};

/// Dispatches user and system messages to actor handlers.
pub trait MessageInvoker<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Processes user messages.
  fn invoke_user_message(&self, message: AnyMessage<TB>) -> Result<(), ActorError>;

  /// Processes system messages.
  fn invoke_system_message(&self, message: SystemMessage) -> Result<(), ActorError>;
}
