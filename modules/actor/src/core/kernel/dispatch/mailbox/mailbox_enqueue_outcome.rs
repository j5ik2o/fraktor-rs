//! Outcome returned by immediate enqueue attempts.

use super::mailbox_offer_future::MailboxOfferFuture;

/// Result of attempting to enqueue a user message without blocking.
#[derive(Debug)]
pub enum EnqueueOutcome {
  /// The message was enqueued immediately.
  Enqueued,
  /// The mailbox is full and a future must be awaited for completion.
  Pending(MailboxOfferFuture),
}
