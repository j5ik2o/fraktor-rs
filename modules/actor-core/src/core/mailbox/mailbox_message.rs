//! Messages dequeued from the mailbox.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::messaging::{AnyMessageGeneric, SystemMessage};

/// Represents messages dequeued from the mailbox.
#[derive(Debug)]
pub enum MailboxMessage<TB: RuntimeToolbox> {
  /// Internal system-level message.
  System(SystemMessage),
  /// Application user-level message.
  User(AnyMessageGeneric<TB>),
}
