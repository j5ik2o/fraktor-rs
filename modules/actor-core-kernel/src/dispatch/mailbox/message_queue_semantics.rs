//! Message queue semantics advertised by mailbox factories.

use crate::actor::props::MailboxRequirement;

/// Describes the queue semantics produced by a mailbox factory.
///
/// This is the Rust representation of Pekko's marker-trait families such as
/// `UnboundedMessageQueueSemantics`, `BoundedMessageQueueSemantics`,
/// `DequeBasedMessageQueueSemantics`, and `MultipleConsumerSemantics`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MessageQueueSemantics {
  bounded:           bool,
  unbounded:         bool,
  deque_based:       bool,
  multiple_consumer: bool,
  control_aware:     bool,
  push_timeout:      bool,
}

impl MessageQueueSemantics {
  /// Creates a semantics set with no marker enabled.
  #[must_use]
  pub const fn none() -> Self {
    Self {
      bounded:           false,
      unbounded:         false,
      deque_based:       false,
      multiple_consumer: false,
      control_aware:     false,
      push_timeout:      false,
    }
  }

  /// Creates unbounded queue semantics.
  #[must_use]
  pub const fn unbounded() -> Self {
    Self { unbounded: true, ..Self::none() }
  }

  /// Creates bounded queue semantics.
  #[must_use]
  pub const fn bounded() -> Self {
    Self { bounded: true, ..Self::none() }
  }

  /// Returns a copy with deque-based semantics enabled or disabled.
  #[must_use]
  pub const fn with_deque_based(mut self, value: bool) -> Self {
    self.deque_based = value;
    self
  }

  /// Returns a copy with multiple-consumer semantics enabled or disabled.
  #[must_use]
  pub const fn with_multiple_consumer(mut self, value: bool) -> Self {
    self.multiple_consumer = value;
    self
  }

  /// Returns a copy with control-aware semantics enabled or disabled.
  #[must_use]
  pub const fn with_control_aware(mut self, value: bool) -> Self {
    self.control_aware = value;
    self
  }

  /// Returns a copy with push-timeout semantics enabled or disabled.
  #[must_use]
  pub const fn with_push_timeout(mut self, value: bool) -> Self {
    self.push_timeout = value;
    self
  }

  /// Returns true when bounded semantics are advertised.
  #[must_use]
  pub const fn is_bounded(&self) -> bool {
    self.bounded
  }

  /// Returns true when unbounded semantics are advertised.
  #[must_use]
  pub const fn is_unbounded(&self) -> bool {
    self.unbounded
  }

  /// Returns true when deque operations are advertised.
  #[must_use]
  pub const fn is_deque_based(&self) -> bool {
    self.deque_based
  }

  /// Returns true when multiple consumers may drain the same queue.
  #[must_use]
  pub const fn is_multiple_consumer(&self) -> bool {
    self.multiple_consumer
  }

  /// Returns true when control-aware prioritisation is advertised.
  #[must_use]
  pub const fn is_control_aware(&self) -> bool {
    self.control_aware
  }

  /// Returns true when push-timeout enqueue semantics are advertised.
  #[must_use]
  pub const fn has_push_timeout(&self) -> bool {
    self.push_timeout
  }

  /// Returns true when this semantics set satisfies `requirement`.
  #[must_use]
  pub const fn satisfies(&self, requirement: MailboxRequirement) -> bool {
    (!requirement.needs_deque() || self.deque_based)
      && (!requirement.needs_control_aware() || self.control_aware)
      && (!requirement.needs_multiple_consumer() || self.multiple_consumer)
      && (!requirement.needs_blocking_future() || self.push_timeout)
  }
}

impl Default for MessageQueueSemantics {
  fn default() -> Self {
    Self::unbounded()
  }
}
