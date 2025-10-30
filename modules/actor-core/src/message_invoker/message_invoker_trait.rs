use crate::{any_message::AnyOwnedMessage, system_message::SystemMessage};

/// Abstraction for delivering messages retrieved from the mailbox to the actor.
pub trait MessageInvoker: Send + Sync {
  /// Processes user messages.
  fn invoke_user_message(&self, message: AnyOwnedMessage);

  /// Processes system messages.
  fn invoke_system_message(&self, message: SystemMessage);
}
