//! Trait for dispatching messages from the mailbox to actors.

use crate::{
  RuntimeToolbox,
  error::ActorError,
  messaging::{AnyMessageGeneric, SystemMessage},
};

/// Dispatches user and system messages to actor handlers.
pub trait MessageInvoker<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Processes user messages.
  ///
  /// # Errors
  ///
  /// Returns an error if message processing fails.
  fn invoke_user_message(&self, message: AnyMessageGeneric<TB>) -> Result<(), ActorError>;

  /// Processes system messages.
  ///
  /// # Errors
  ///
  /// Returns an error if system message processing fails.
  fn invoke_system_message(&self, message: SystemMessage) -> Result<(), ActorError>;
}
