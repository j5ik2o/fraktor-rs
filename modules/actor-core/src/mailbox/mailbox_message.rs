use crate::{any_message::AnyMessage, system_message::SystemMessage};

/// Represents messages dequeued from the mailbox.
#[derive(Debug)]
pub enum MailboxMessage {
  /// Internal system-level message.
  System(SystemMessage),
  /// Application user-level message.
  User(AnyMessage),
}
