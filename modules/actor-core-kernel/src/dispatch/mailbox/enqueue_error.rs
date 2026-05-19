//! Error reported by a failed
//! [`MessageQueue::enqueue`](super::message_queue::MessageQueue::enqueue).
//!
//! Carries the failing [`SendError`] together with an optional `evicted`
//! envelope that was displaced from the queue before the enqueue failed.
//! This is required for Pekko parity: [`MailboxOverflowStrategy::DropOldest`]
//! may evict an existing envelope from the underlying queue even when the
//! subsequent offer of the new envelope fails (e.g. the backend is closed in
//! a race). The mailbox layer must forward any `evicted` envelope to the
//! dead-letter destination instead of silently discarding it.
//!
//! The large [`SendError`] variants are boxed so `Result<EnqueueOutcome,
//! EnqueueError>` stays small on the happy path (see clippy's
//! `result_large_err`).
//!
//! [`MailboxOverflowStrategy::DropOldest`]: super::overflow_strategy::MailboxOverflowStrategy

use alloc::boxed::Box;

use super::envelope::Envelope;
use crate::actor::error::SendError;

/// Error surfaced when an enqueue operation fails.
#[derive(Debug)]
pub struct EnqueueError {
  /// Underlying send failure describing why the new envelope was rejected.
  ///
  /// Boxed to keep the error small on the hot enqueue path.
  error:   Box<SendError>,
  /// Envelope evicted before the failing enqueue (present when a
  /// `DropOldest` round still displaced an existing message even though
  /// the subsequent offer failed). Boxed for the same reason as `error`.
  evicted: Option<Box<Envelope>>,
}

impl EnqueueError {
  /// Creates an [`EnqueueError`] carrying only the underlying send failure.
  #[must_use]
  pub fn new(error: SendError) -> Self {
    Self { error: Box::new(error), evicted: None }
  }

  /// Creates an [`EnqueueError`] that also surfaces an evicted envelope so the
  /// mailbox layer can route it to dead letters.
  #[must_use]
  pub fn with_evicted(error: SendError, evicted: Envelope) -> Self {
    Self { error: Box::new(error), evicted: Some(Box::new(evicted)) }
  }

  /// Returns the underlying send error.
  #[must_use]
  pub fn error(&self) -> &SendError {
    &self.error
  }

  /// Returns the evicted envelope, if any.
  #[must_use]
  pub fn evicted(&self) -> Option<&Envelope> {
    self.evicted.as_deref()
  }

  /// Consumes the error and returns its components: the send error and any
  /// evicted envelope that must be routed to dead letters.
  #[must_use]
  pub fn into_parts(self) -> (SendError, Option<Envelope>) {
    (*self.error, self.evicted.map(|evicted| *evicted))
  }
}

impl From<SendError> for EnqueueError {
  fn from(error: SendError) -> Self {
    Self::new(error)
  }
}
