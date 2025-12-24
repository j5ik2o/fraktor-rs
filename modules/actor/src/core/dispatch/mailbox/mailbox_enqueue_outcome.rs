//! Outcome returned by immediate enqueue attempts.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::mailbox_offer_future::MailboxOfferFutureGeneric;

/// Result of attempting to enqueue a user message without blocking.
#[derive(Debug)]
pub enum EnqueueOutcome<TB: RuntimeToolbox + 'static> {
  /// The message was enqueued immediately.
  Enqueued,
  /// The mailbox is full and a future must be awaited for completion.
  Pending(MailboxOfferFutureGeneric<TB>),
}
