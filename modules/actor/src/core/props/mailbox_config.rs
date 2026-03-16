use core::num::NonZeroUsize;

use fraktor_utils_rs::core::{collections::queue::capabilities::QueueCapabilityRegistry, sync::ArcShared};

use super::{MailboxConfigError, MailboxRequirement};
use crate::core::dispatch::mailbox::{MailboxPolicy, MessagePriorityGenerator};

#[cfg(test)]
mod tests;

/// Mailbox configuration derived from the props builder.
///
/// When a [`MessagePriorityGenerator`] is attached, the mailbox selection logic
/// produces a priority-based message queue instead of the default FIFO queue.
/// When `stable_priority` is also set, equal-priority messages are dequeued in
/// FIFO (insertion) order.
#[derive(Clone)]
pub struct MailboxConfig {
  policy:             MailboxPolicy,
  warn_threshold:     Option<NonZeroUsize>,
  requirement:        MailboxRequirement,
  capabilities:       QueueCapabilityRegistry,
  priority_generator: Option<ArcShared<dyn MessagePriorityGenerator>>,
  stable_priority:    bool,
}

impl MailboxConfig {
  /// Creates a new mailbox configuration.
  #[must_use]
  pub fn new(policy: MailboxPolicy) -> Self {
    Self {
      policy,
      warn_threshold: None,
      requirement: MailboxRequirement::none(),
      capabilities: QueueCapabilityRegistry::with_defaults(),
      priority_generator: None,
      stable_priority: false,
    }
  }

  /// Returns the mailbox policy.
  #[must_use]
  pub const fn policy(&self) -> MailboxPolicy {
    self.policy
  }

  /// Returns the warning threshold.
  #[must_use]
  pub const fn warn_threshold(&self) -> Option<NonZeroUsize> {
    self.warn_threshold
  }

  /// Returns the mailbox requirement description.
  #[must_use]
  pub const fn requirement(&self) -> MailboxRequirement {
    self.requirement
  }

  /// Returns the configured capability registry.
  #[must_use]
  pub const fn capabilities(&self) -> QueueCapabilityRegistry {
    self.capabilities
  }

  /// Returns the priority generator, if any.
  #[must_use]
  pub fn priority_generator(&self) -> Option<&ArcShared<dyn MessagePriorityGenerator>> {
    self.priority_generator.as_ref()
  }

  /// Returns whether stable ordering is enabled for priority queues.
  #[must_use]
  pub const fn stable_priority(&self) -> bool {
    self.stable_priority
  }

  /// Updates the warning threshold.
  #[must_use]
  pub const fn with_warn_threshold(mut self, threshold: Option<NonZeroUsize>) -> Self {
    self.warn_threshold = threshold;
    self
  }

  /// Overrides the mailbox requirement set.
  #[must_use]
  pub const fn with_requirement(mut self, requirement: MailboxRequirement) -> Self {
    self.requirement = requirement;
    self
  }

  /// Overrides the capability registry used to validate requirements.
  #[must_use]
  pub const fn with_capabilities(mut self, registry: QueueCapabilityRegistry) -> Self {
    self.capabilities = registry;
    self
  }

  /// Attaches a priority generator to produce priority-based message queues.
  #[must_use]
  pub fn with_priority_generator(mut self, generator: ArcShared<dyn MessagePriorityGenerator>) -> Self {
    self.priority_generator = Some(generator);
    self
  }

  /// Enables stable ordering for priority queues.
  ///
  /// When enabled, messages with equal priority are dequeued in FIFO
  /// (insertion) order. Requires a priority generator to be attached.
  #[must_use]
  pub const fn with_stable_priority(mut self, stable: bool) -> Self {
    self.stable_priority = stable;
    self
  }

  /// Validates the configuration contract.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxConfigError::StablePriorityWithoutGenerator`] when
  /// `stable_priority` is enabled but no priority generator has been attached.
  ///
  /// Returns [`MailboxConfigError::ControlAwareRequiresUnboundedPolicy`] when
  /// `needs_control_aware()` is set but the policy is bounded.
  ///
  /// Returns [`MailboxConfigError::PriorityWithControlAware`] when both a
  /// priority generator and control-aware semantics are requested simultaneously.
  ///
  /// Returns [`MailboxConfigError::BoundedWithDeque`] when the policy is bounded
  /// and the requirement needs deque semantics (not supported).
  pub fn validate(&self) -> Result<(), MailboxConfigError> {
    if self.stable_priority && self.priority_generator.is_none() {
      return Err(MailboxConfigError::StablePriorityWithoutGenerator);
    }
    if self.requirement.needs_control_aware()
      && matches!(self.policy.capacity(), crate::core::dispatch::mailbox::MailboxCapacity::Bounded { .. })
    {
      return Err(MailboxConfigError::ControlAwareRequiresUnboundedPolicy);
    }
    if self.priority_generator.is_some() && self.requirement.needs_control_aware() {
      return Err(MailboxConfigError::PriorityWithControlAware);
    }
    if self.requirement.needs_deque()
      && matches!(self.policy.capacity(), crate::core::dispatch::mailbox::MailboxCapacity::Bounded { .. })
    {
      return Err(MailboxConfigError::BoundedWithDeque);
    }
    Ok(())
  }
}

impl Default for MailboxConfig {
  fn default() -> Self {
    MailboxConfig::new(MailboxPolicy::unbounded(None))
  }
}

impl core::fmt::Debug for MailboxConfig {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("MailboxConfig")
      .field("policy", &self.policy)
      .field("warn_threshold", &self.warn_threshold)
      .field("requirement", &self.requirement)
      .field("capabilities", &self.capabilities)
      .field("has_priority_generator", &self.priority_generator.is_some())
      .field("stable_priority", &self.stable_priority)
      .finish()
  }
}
