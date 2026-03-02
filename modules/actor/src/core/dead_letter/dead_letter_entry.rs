//! Entry stored in the deadletter repository.

use core::time::Duration;

use crate::core::{actor::Pid, dead_letter::dead_letter_reason::DeadLetterReason, messaging::AnyMessage};

/// Captures a single deadletter occurrence.
#[derive(Debug)]
pub struct DeadLetterEntry {
  message:   AnyMessage,
  reason:    DeadLetterReason,
  recipient: Option<Pid>,
  timestamp: Duration,
}

impl DeadLetterEntry {
  /// Creates a new deadletter entry.
  #[must_use]
  pub const fn new(message: AnyMessage, reason: DeadLetterReason, recipient: Option<Pid>, timestamp: Duration) -> Self {
    Self { message, reason, recipient, timestamp }
  }

  /// Returns the undelivered message.
  #[must_use]
  pub const fn message(&self) -> &AnyMessage {
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

impl Clone for DeadLetterEntry {
  fn clone(&self) -> Self {
    Self {
      message:   self.message.clone(),
      reason:    self.reason,
      recipient: self.recipient,
      timestamp: self.timestamp,
    }
  }
}
