//! Trait for dispatching messages from the mailbox to actors.

use crate::core::kernel::{
  actor::{
    error::ActorError,
    messaging::{AnyMessage, system_message::SystemMessage},
  },
  dispatch::mailbox::metrics_event::MailboxPressureEvent,
};

/// Dispatches user and system messages to actor handlers.
///
/// Implementations should be wrapped in [`MessageInvokerShared`](super::MessageInvokerShared)
/// for shared access using `with_write`:
///
/// ```text
/// let invoker = MessageInvokerShared::new(boxed_invoker);
/// invoker.with_write(|i| i.invoke(message))?;
/// ```
pub trait MessageInvoker: Send + Sync {
  /// Processes a user message, mirroring Pekko `ActorCell.scala:548` `invoke(Envelope)`.
  ///
  /// # Errors
  ///
  /// Returns an error if message processing fails.
  fn invoke(&mut self, message: AnyMessage) -> Result<(), ActorError>;

  /// Processes a system message, mirroring Pekko `ActorCell.scala:480` `systemInvoke(SystemMessage)`.
  ///
  /// # Errors
  ///
  /// Returns an error if system message processing fails.
  fn system_invoke(&mut self, message: SystemMessage) -> Result<(), ActorError>;

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
