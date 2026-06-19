//! Helpers for bounded mailbox push timeouts.

use super::{EnqueueError, Envelope};
use crate::actor::error::SendError;

pub(crate) fn enqueue_timeout(envelope: Envelope) -> EnqueueError {
  EnqueueError::new(send_timeout(envelope))
}

pub(crate) fn send_timeout(envelope: Envelope) -> SendError {
  SendError::timeout(envelope.into_payload())
}
