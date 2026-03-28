//! Messages dequeued from the mailbox.

use crate::core::kernel::messaging::{AnyMessage, system_message::SystemMessage};

/// Represents messages dequeued from the mailbox.
#[derive(Debug)]
pub(crate) enum MailboxMessage {
  /// Internal system-level message.
  System(SystemMessage),
  /// Application user-level message.
  User(AnyMessage),
}
