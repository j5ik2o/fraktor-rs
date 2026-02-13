//! Trait for dispatching messages from the mailbox to actors.

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{
  dispatch::mailbox::metrics_event::MailboxPressureEvent,
  error::ActorError,
  messaging::{AnyMessageGeneric, system_message::SystemMessage},
};

/// Dispatches user and system messages to actor handlers.
///
/// Implementations should be wrapped in [`MessageInvokerShared`](super::MessageInvokerShared)
/// for shared access using `with_write`:
///
/// ```text
/// let invoker = MessageInvokerShared::new(boxed_invoker);
/// invoker.with_write(|i| i.invoke_user_message(message))?;
/// ```
pub trait MessageInvoker<TB: RuntimeToolbox + 'static = NoStdToolbox>: Send + Sync {
  /// Processes user messages.
  ///
  /// # Errors
  ///
  /// Returns an error if message processing fails.
  fn invoke_user_message(&mut self, message: AnyMessageGeneric<TB>) -> Result<(), ActorError>;

  /// Processes system messages.
  ///
  /// # Errors
  ///
  /// Returns an error if system message processing fails.
  fn invoke_system_message(&mut self, message: SystemMessage) -> Result<(), ActorError>;

  /// Processes mailbox-pressure notifications emitted by dispatcher instrumentation.
  ///
  /// # Errors
  ///
  /// Returns an error if pressure handling fails.
  #[allow(unused_variables)]
  fn invoke_mailbox_pressure(&mut self, event: &MailboxPressureEvent) -> Result<(), ActorError> {
    Ok(())
  }
}
