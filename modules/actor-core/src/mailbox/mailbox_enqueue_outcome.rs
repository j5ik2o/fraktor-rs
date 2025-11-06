//! Outcome returned by immediate enqueue attempts.

use super::mailbox_offer_future::MailboxOfferFutureGeneric;
use crate::RuntimeToolbox;

/// Result of attempting to enqueue a user message without blocking.
#[derive(Debug)]
pub enum EnqueueOutcome<TB: RuntimeToolbox + 'static> {
  /// The message was enqueued immediately.
  Enqueued,
  /// The mailbox is full and a future must be awaited for completion.
  Pending(MailboxOfferFutureGeneric<TB>),
}
