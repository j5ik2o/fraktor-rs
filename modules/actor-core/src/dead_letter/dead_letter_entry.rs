//! Entry stored in the deadletter repository.

use core::time::Duration;

use crate::{
  NoStdToolbox, RuntimeToolbox, actor_prim::Pid, dead_letter::dead_letter_reason::DeadLetterReason,
  messaging::AnyMessage,
};

/// Captures a single deadletter occurrence.
#[derive(Debug)]
pub struct DeadLetterEntry<TB: RuntimeToolbox = NoStdToolbox> {
  message:   AnyMessage<TB>,
  reason:    DeadLetterReason,
  recipient: Option<Pid>,
  timestamp: Duration,
}

impl<TB: RuntimeToolbox> DeadLetterEntry<TB> {
  /// Creates a new deadletter entry.
  #[must_use]
  pub const fn new(
    message: AnyMessage<TB>,
    reason: DeadLetterReason,
    recipient: Option<Pid>,
    timestamp: Duration,
  ) -> Self {
    Self { message, reason, recipient, timestamp }
  }

  /// Returns the undelivered message.
  #[must_use]
  pub const fn message(&self) -> &AnyMessage<TB> {
    &self.message
  }

  /// Returns the deadletter reason.
  #[must_use]
  pub const fn reason(&self) -> DeadLetterReason {
    self.reason
  }

  /// Returns the intended recipient pid.
  #[must_use]
  pub const fn recipient(&self) -> Option<Pid> {
    self.recipient
  }

  /// Returns the timestamp.
  #[must_use]
  pub const fn timestamp(&self) -> Duration {
    self.timestamp
  }
}

impl<TB: RuntimeToolbox> Clone for DeadLetterEntry<TB> {
  fn clone(&self) -> Self {
    Self {
      message:   self.message.clone(),
      reason:    self.reason,
      recipient: self.recipient,
      timestamp: self.timestamp,
    }
  }
}
