use super::super::QueueError;

/// Enumerates queue capabilities required by higher-level components.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueCapability {
  /// Multi-producer single-consumer MPSC semantics.
  Mpsc,
  /// Double-ended deque operations.
  Deque,
  /// Futures that wait for capacity (blocking offer/poll).
  BlockingFuture,
}

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

/// Describes the capability set available at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueCapabilitySet {
  has_mpsc:            bool,
  has_deque:           bool,
  has_blocking_future: bool,
}

impl QueueCapabilitySet {
  /// Creates a fully disabled capability set.
  pub const fn empty() -> Self {
    Self { has_mpsc: false, has_deque: false, has_blocking_future: false }
  }

  /// Creates a capability set with all runtime-provided defaults enabled.
  pub const fn defaults() -> Self {
    Self { has_mpsc: true, has_deque: true, has_blocking_future: true }
  }

  /// Enables the MPSC capability flag.
  #[must_use]
  pub const fn with_mpsc(mut self, value: bool) -> Self {
    self.has_mpsc = value;
    self
  }

  /// Enables the deque capability flag.
  #[must_use]
  pub const fn with_deque(mut self, value: bool) -> Self {
    self.has_deque = value;
    self
  }

  /// Enables the blocking future capability flag.
  #[must_use]
  pub const fn with_blocking_future(mut self, value: bool) -> Self {
    self.has_blocking_future = value;
    self
  }

  const fn has(&self, capability: QueueCapability) -> bool {
    match capability {
      | QueueCapability::Mpsc => self.has_mpsc,
      | QueueCapability::Deque => self.has_deque,
      | QueueCapability::BlockingFuture => self.has_blocking_future,
    }
  }
}

impl Default for QueueCapabilitySet {
  fn default() -> Self {
    Self::defaults()
  }
}

/// Registry that validates queue capability availability.
#[derive(Clone, Debug, PartialEq, Eq)]
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

  /// Ensures the provided capability exists, returning an error when it does not.
  pub fn ensure(&self, capability: QueueCapability) -> Result<(), QueueCapabilityError> {
    if self.set.has(capability) { Ok(()) } else { Err(QueueCapabilityError::new(capability)) }
  }

  /// Ensures all capabilities in the provided slice exist.
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

impl core::fmt::Display for QueueCapabilityError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "missing queue capability: {:?}", self.missing)
  }
}

impl core::fmt::Display for QueueCapabilityRegistry {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "QueueCapabilityRegistry")
  }
}

impl<T> From<QueueCapabilityError> for QueueError<T> {
  fn from(_value: QueueCapabilityError) -> Self {
    QueueError::Disconnected
  }
}
