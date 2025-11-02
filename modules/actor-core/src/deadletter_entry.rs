//! Entry stored in the deadletter repository.

use core::time::Duration;

use crate::{AnyMessage, DeadletterReason, Pid, RuntimeToolbox};

/// Captures a single deadletter occurrence.
#[derive(Debug)]
pub struct DeadletterEntry<TB: RuntimeToolbox> {
  message:   AnyMessage<TB>,
  reason:    DeadletterReason,
  recipient: Option<Pid>,
  timestamp: Duration,
}

impl<TB: RuntimeToolbox> DeadletterEntry<TB> {
  /// Creates a new deadletter entry.
  #[must_use]
  pub fn new(message: AnyMessage<TB>, reason: DeadletterReason, recipient: Option<Pid>, timestamp: Duration) -> Self {
    Self { message, reason, recipient, timestamp }
  }

  /// Returns the undelivered message.
  #[must_use]
  pub fn message(&self) -> &AnyMessage<TB> {
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

impl<TB: RuntimeToolbox> Clone for DeadletterEntry<TB> {
  fn clone(&self) -> Self {
    Self {
      message:   self.message.clone(),
      reason:    self.reason,
      recipient: self.recipient,
      timestamp: self.timestamp,
    }
  }
}
