//! Actor-side mailbox requirement marker.

use super::MailboxRequirement;

/// Marker trait for actors that require specific message queue semantics.
///
/// This is the Rust counterpart of Pekko's `RequiresMessageQueue[T]`. Actor
/// implementations can expose a static requirement, then callers can apply it
/// to [`Props`](super::Props) with
/// [`Props::with_required_message_queue`](super::Props::with_required_message_queue)
/// or create props through
/// [`Props::from_required_fn`](super::Props::from_required_fn).
pub trait RequiresMessageQueue {
  /// Returns the queue semantics required by this actor type.
  #[must_use]
  fn required_message_queue() -> MailboxRequirement;
}
