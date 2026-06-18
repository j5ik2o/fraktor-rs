use fraktor_utils_core_rs::collections::queue::capabilities::{
  QueueCapability, QueueCapabilityError, QueueCapabilityRegistry,
};

#[cfg(test)]
#[path = "mailbox_requirement_test.rs"]
mod tests;

/// Declares mailbox-level requirements such as deque or blocking futures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MailboxRequirement {
  requires_deque:             bool,
  requires_blocking_future:   bool,
  requires_control_aware:     bool,
  requires_multiple_consumer: bool,
}

impl MailboxRequirement {
  /// Creates a requirement set with no capabilities.
  #[must_use]
  pub const fn none() -> Self {
    Self {
      requires_deque:             false,
      requires_blocking_future:   false,
      requires_control_aware:     false,
      requires_multiple_consumer: false,
    }
  }

  /// Convenience alias for a stash-compatible requirement (deque only for now).
  #[must_use]
  pub const fn for_stash() -> Self {
    Self::requires_deque()
  }

  /// Creates a requirement that needs deque semantics.
  #[must_use]
  pub const fn requires_deque() -> Self {
    Self {
      requires_deque:             true,
      requires_blocking_future:   false,
      requires_control_aware:     false,
      requires_multiple_consumer: false,
    }
  }

  /// Creates a requirement that needs control-aware semantics.
  #[must_use]
  pub const fn requires_control_aware() -> Self {
    Self {
      requires_deque:             false,
      requires_blocking_future:   false,
      requires_control_aware:     true,
      requires_multiple_consumer: false,
    }
  }

  /// Creates a requirement that needs multiple-consumer queue semantics.
  #[must_use]
  pub const fn requires_multiple_consumer() -> Self {
    Self {
      requires_deque:             false,
      requires_blocking_future:   false,
      requires_control_aware:     false,
      requires_multiple_consumer: true,
    }
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

  /// Marks the requirement as needing multiple-consumer semantics.
  #[must_use]
  pub const fn with_multiple_consumer(mut self) -> Self {
    self.requires_multiple_consumer = true;
    self
  }

  /// Returns true when no queue semantics are required.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    !self.requires_deque
      && !self.requires_blocking_future
      && !self.requires_control_aware
      && !self.requires_multiple_consumer
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

  /// Returns true when multiple consumers may drain the same queue.
  #[must_use]
  pub const fn needs_multiple_consumer(&self) -> bool {
    self.requires_multiple_consumer
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
