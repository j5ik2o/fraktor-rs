//! Entry stored in the deadletter repository.

use core::time::Duration;

use crate::{any_message::AnyMessage, deadletter_reason::DeadletterReason, pid::Pid};

/// Captures a single deadletter occurrence.
#[derive(Clone, Debug)]
pub struct DeadletterEntry {
  message:   AnyMessage,
  reason:    DeadletterReason,
  recipient: Option<Pid>,
  timestamp: Duration,
}

impl DeadletterEntry {
  /// Creates a new deadletter entry.
  #[must_use]
  pub const fn new(message: AnyMessage, reason: DeadletterReason, recipient: Option<Pid>, timestamp: Duration) -> Self {
    Self { message, reason, recipient, timestamp }
  }

  /// Returns the undelivered message.
  #[must_use]
  pub const fn message(&self) -> &AnyMessage {
    &self.message
  }

  /// Returns the deadletter reason.
  #[must_use]
  pub const fn reason(&self) -> DeadletterReason {
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
