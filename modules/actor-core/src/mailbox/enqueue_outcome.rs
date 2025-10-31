use super::mailbox_offer_future::MailboxOfferFuture;

/// Outcome returned by immediate enqueue attempts.
#[derive(Debug)]
pub enum EnqueueOutcome {
  /// The message was enqueued immediately.
  Enqueued,
  /// The mailbox is full and a future must be awaited for completion.
  Pending(MailboxOfferFuture),
}
