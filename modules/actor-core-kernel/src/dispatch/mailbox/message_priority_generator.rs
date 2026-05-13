//! Priority generator for priority-based message queues.

use crate::actor::messaging::AnyMessage;

/// Determines the priority of a user message for priority-based mailboxes.
///
/// Inspired by Pekko's `Comparator[Envelope]` parameter used in
/// `UnboundedPriorityMailbox` / `BoundedPriorityMailbox`.
///
/// Lower values indicate higher priority (dequeued first).
pub trait MessagePriorityGenerator: Send + Sync {
  /// Returns the priority value for the given message.
  ///
  /// Lower values are dequeued before higher values.
  fn priority(&self, message: &AnyMessage) -> i32;
}

/// Blanket implementation for closures.
impl<F> MessagePriorityGenerator for F
where
  F: Fn(&AnyMessage) -> i32 + Send + Sync,
{
  fn priority(&self, message: &AnyMessage) -> i32 {
    self(message)
  }
}
