//! Entry stored in the deadletter repository.

use core::time::Duration;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{actor_prim::Pid, dead_letter::dead_letter_reason::DeadLetterReason, messaging::AnyMessageGeneric};

/// Captures a single deadletter occurrence.
#[derive(Debug)]
pub struct DeadLetterEntryGeneric<TB: RuntimeToolbox> {
  message:   AnyMessageGeneric<TB>,
  reason:    DeadLetterReason,
  recipient: Option<Pid>,
  timestamp: Duration,
}

impl<TB: RuntimeToolbox> DeadLetterEntryGeneric<TB> {
  /// Creates a new deadletter entry.
  #[must_use]
  pub const fn new(
    message: AnyMessageGeneric<TB>,
    reason: DeadLetterReason,
    recipient: Option<Pid>,
    timestamp: Duration,
  ) -> Self {
    Self { message, reason, recipient, timestamp }
  }

  /// Returns the undelivered message.
  #[must_use]
  pub const fn message(&self) -> &AnyMessageGeneric<TB> {
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

impl<TB: RuntimeToolbox> Clone for DeadLetterEntryGeneric<TB> {
  fn clone(&self) -> Self {
    Self {
      message:   self.message.clone(),
      reason:    self.reason,
      recipient: self.recipient,
      timestamp: self.timestamp,
    }
  }
}

/// Type alias for `DeadLetterEntryGeneric` with the default `NoStdToolbox`.
pub type DeadLetterEntry = DeadLetterEntryGeneric<NoStdToolbox>;
