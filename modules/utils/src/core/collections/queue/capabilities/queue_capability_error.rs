use super::{super::QueueError, QueueCapability};

/// Error returned when required capabilities are missing from the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueCapabilityError {
  missing: QueueCapability,
}

impl QueueCapabilityError {
  /// Creates a new error describing the missing capability.
  #[must_use]
  pub const fn new(missing: QueueCapability) -> Self {
    Self { missing }
  }

  /// Returns the missing capability identifier.
  #[must_use]
  pub const fn missing(&self) -> QueueCapability {
    self.missing
  }
}

impl core::fmt::Display for QueueCapabilityError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "missing queue capability: {:?}", self.missing)
  }
}

impl<T> From<QueueCapabilityError> for QueueError<T> {
  fn from(_value: QueueCapabilityError) -> Self {
    QueueError::Disconnected
  }
}
