//! Mailbox-side produced queue marker.

use crate::dispatch::mailbox::{MailboxFactory, MessageQueueSemantics};

/// Marker trait for factories that advertise produced queue semantics.
///
/// This mirrors Pekko's `ProducesMessageQueue[T]` without relying on runtime
/// reflection. Implementations normally expose semantics through
/// [`MailboxFactory::produced_queue_semantics`].
pub trait ProducesMessageQueue {
  /// Returns the produced queue semantics.
  #[must_use]
  fn produced_message_queue(&self) -> MessageQueueSemantics;
}

impl<T> ProducesMessageQueue for T
where
  T: MailboxFactory + ?Sized,
{
  fn produced_message_queue(&self) -> MessageQueueSemantics {
    self.produced_queue_semantics()
  }
}
