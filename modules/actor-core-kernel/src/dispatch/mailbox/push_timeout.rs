//! Helpers for Pekko-style bounded mailbox push timeouts.

use core::{hint::spin_loop, time::Duration};

use super::{EnqueueError, Envelope, MailboxClock};
use crate::actor::error::SendError;

pub(crate) fn push_timeout_deadline(clock: &MailboxClock, timeout: Duration) -> Duration {
  clock().saturating_add(timeout)
}

pub(crate) fn should_retry_after_full(clock: &MailboxClock, deadline: Duration) -> bool {
  clock() < deadline
}

pub(crate) fn spin_before_push_timeout_retry() {
  spin_loop();
}

pub(crate) fn enqueue_timeout(envelope: Envelope) -> EnqueueError {
  EnqueueError::new(send_timeout(envelope))
}

pub(crate) fn send_timeout(envelope: Envelope) -> SendError {
  SendError::timeout(envelope.into_payload())
}
