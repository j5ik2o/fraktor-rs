use super::QueueCapability;

/// Describes the capability set available at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueCapabilitySet {
  has_mpsc:            bool,
  has_deque:           bool,
  has_blocking_future: bool,
  has_control_aware:   bool,
}

impl QueueCapabilitySet {
  /// Creates a fully disabled capability set.
  #[must_use]
  pub const fn empty() -> Self {
    Self { has_mpsc: false, has_deque: false, has_blocking_future: false, has_control_aware: false }
  }

  /// Creates a capability set with all runtime-provided defaults enabled.
  #[must_use]
  pub const fn defaults() -> Self {
    Self { has_mpsc: true, has_deque: true, has_blocking_future: true, has_control_aware: true }
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

  /// Enables the control-aware capability flag.
  #[must_use]
  pub const fn with_control_aware(mut self, value: bool) -> Self {
    self.has_control_aware = value;
    self
  }

  #[must_use]
  pub(crate) const fn has(self, capability: QueueCapability) -> bool {
    match capability {
      | QueueCapability::Mpsc => self.has_mpsc,
      | QueueCapability::Deque => self.has_deque,
      | QueueCapability::BlockingFuture => self.has_blocking_future,
      | QueueCapability::ControlAware => self.has_control_aware,
    }
  }
}

impl Default for QueueCapabilitySet {
  fn default() -> Self {
    Self::defaults()
  }
}
