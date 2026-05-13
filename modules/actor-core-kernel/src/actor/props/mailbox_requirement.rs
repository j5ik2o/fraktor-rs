use fraktor_utils_core_rs::collections::queue::capabilities::{
  QueueCapability, QueueCapabilityError, QueueCapabilityRegistry,
};

#[cfg(test)]
#[path = "mailbox_requirement_test.rs"]
mod tests;

/// Declares mailbox-level requirements such as deque or blocking futures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailboxRequirement {
  requires_deque:           bool,
  requires_blocking_future: bool,
  requires_control_aware:   bool,
}

impl MailboxRequirement {
  /// Creates a requirement set with no capabilities.
  #[must_use]
  pub const fn none() -> Self {
    Self { requires_deque: false, requires_blocking_future: false, requires_control_aware: false }
  }

  /// Convenience alias for a stash-compatible requirement (deque only for now).
  #[must_use]
  pub const fn for_stash() -> Self {
    Self::requires_deque()
  }

  /// Creates a requirement that needs deque semantics.
  #[must_use]
  pub const fn requires_deque() -> Self {
    Self { requires_deque: true, requires_blocking_future: false, requires_control_aware: false }
  }

  /// Creates a requirement that needs control-aware semantics.
  #[must_use]
  pub const fn requires_control_aware() -> Self {
    Self { requires_deque: false, requires_blocking_future: false, requires_control_aware: true }
  }

  /// Marks the requirement as needing deque semantics.
  #[must_use]
  pub const fn with_deque(mut self) -> Self {
    self.requires_deque = true;
    self
  }

  /// Marks the requirement as needing blocking future semantics.
  #[must_use]
  pub const fn with_blocking_future(mut self) -> Self {
    self.requires_blocking_future = true;
    self
  }

  /// Marks the requirement as needing control-aware semantics.
  #[must_use]
  pub const fn with_control_aware(mut self) -> Self {
    self.requires_control_aware = true;
    self
  }

  /// Returns true when deque operations are required.
  #[must_use]
  pub const fn needs_deque(&self) -> bool {
    self.requires_deque
  }

  /// Returns true when blocking futures are required.
  #[must_use]
  pub const fn needs_blocking_future(&self) -> bool {
    self.requires_blocking_future
  }

  /// Returns true when control-aware mailbox semantics are required.
  #[must_use]
  pub const fn needs_control_aware(&self) -> bool {
    self.requires_control_aware
  }

  /// Ensures all declared requirements are supported by the registry.
  ///
  /// # Errors
  ///
  /// Returns [`QueueCapabilityError`] when the provided registry misses one of the required
  /// capabilities.
  pub fn ensure_supported(&self, registry: &QueueCapabilityRegistry) -> Result<(), QueueCapabilityError> {
    if self.requires_deque {
      registry.ensure(QueueCapability::Deque)?;
    }
    if self.requires_blocking_future {
      registry.ensure(QueueCapability::BlockingFuture)?;
    }
    if self.requires_control_aware {
      registry.ensure(QueueCapability::ControlAware)?;
    }
    Ok(())
  }
}

impl Default for MailboxRequirement {
  fn default() -> Self {
    Self::none()
  }
}
