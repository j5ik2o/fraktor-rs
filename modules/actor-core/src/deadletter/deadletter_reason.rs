//! Reasons captured when messages are routed to deadletter storage.

/// High level classification explaining why a message was not delivered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeadletterReason {
  /// Mailbox capacity or overflow strategy rejected the message.
  MailboxFull,
  /// Mailbox was suspended at the time of delivery.
  MailboxSuspended,
  /// Mailbox or actor was closed, or the recipient pid did not exist.
  RecipientUnavailable,
  /// No reply target was present for an ask/tell expecting a response.
  MissingRecipient,
  /// Actor execution failed with a fatal error.
  FatalActorError,
  /// Message was explicitly redirected to deadletter by system logic.
  ExplicitRouting,
}
