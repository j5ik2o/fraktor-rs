//! Messages dequeued from the mailbox.

use super::envelope::Envelope;
use crate::core::kernel::actor::messaging::system_message::SystemMessage;

/// Represents messages dequeued from the mailbox.
#[derive(Debug)]
pub enum MailboxMessage {
  /// Internal system-level message.
  System(SystemMessage),
  /// Application user-level envelope.
  User(Envelope),
}
