use super::{QueueCapability, QueueCapabilityError, QueueCapabilitySet};

/// Registry that validates queue capability availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueCapabilityRegistry {
  set: QueueCapabilitySet,
}

impl QueueCapabilityRegistry {
  /// Creates a new registry with the provided capability set.
  #[must_use]
  pub const fn new(set: QueueCapabilitySet) -> Self {
    Self { set }
  }

  /// Returns a registry populated with the default capability detection.
  #[must_use]
  pub const fn with_defaults() -> Self {
    Self::new(QueueCapabilitySet::defaults())
  }

  /// Ensures the provided capability exists.
  ///
  /// # Errors
  ///
  /// Returns [`QueueCapabilityError`] when the capability is missing from the registry.
  pub const fn ensure(&self, capability: QueueCapability) -> Result<(), QueueCapabilityError> {
    if self.set.has(capability) { Ok(()) } else { Err(QueueCapabilityError::new(capability)) }
  }

  /// Ensures all capabilities in the provided slice exist.
  ///
  /// # Errors
  ///
  /// Returns an error when any capability is missing.
  pub fn ensure_all(&self, capabilities: &[QueueCapability]) -> Result<(), QueueCapabilityError> {
    for capability in capabilities {
      self.ensure(*capability)?;
    }
    Ok(())
  }
}

impl Default for QueueCapabilityRegistry {
  fn default() -> Self {
    Self::with_defaults()
  }
}

impl core::fmt::Display for QueueCapabilityRegistry {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "QueueCapabilityRegistry")
  }
}
