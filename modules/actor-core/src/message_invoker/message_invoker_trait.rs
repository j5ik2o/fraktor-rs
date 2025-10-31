use crate::{actor_error::ActorError, any_message::AnyMessage, system_message::SystemMessage};

/// Abstraction for delivering messages retrieved from the mailbox to the actor.
pub trait MessageInvoker: Send + Sync {
  /// Processes user messages.
  ///
  /// # Errors
  ///
  /// Returns an error if the message processing fails or if the actor's handler returns an error.
  fn invoke_user_message(&self, message: AnyMessage) -> Result<(), ActorError>;

  /// Processes system messages.
  ///
  /// # Errors
  ///
  /// Returns an error if the system message processing fails.
  fn invoke_system_message(&self, message: SystemMessage) -> Result<(), ActorError>;
}
