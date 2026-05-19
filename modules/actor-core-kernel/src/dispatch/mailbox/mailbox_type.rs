//! Factory trait for creating message queue instances.

use alloc::boxed::Box;

use super::message_queue::MessageQueue;

/// Factory that produces [`MessageQueue`] instances for new mailboxes.
///
/// Inspired by Pekko's `MailboxType`. Each implementation encapsulates the
/// configuration needed to construct a particular kind of message queue.
pub trait MailboxType: Send + Sync {
  /// Creates a new message queue instance.
  fn create(&self) -> Box<dyn MessageQueue>;
}
