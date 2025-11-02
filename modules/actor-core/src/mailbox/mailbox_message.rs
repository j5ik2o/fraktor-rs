//! Messages dequeued from the mailbox.

use crate::{AnyMessage, RuntimeToolbox, SystemMessage};

/// Represents messages dequeued from the mailbox.
#[derive(Debug)]
pub enum MailboxMessage<TB: RuntimeToolbox> {
  /// Internal system-level message.
  System(SystemMessage),
  /// Application user-level message.
  User(AnyMessage<TB>),
}
