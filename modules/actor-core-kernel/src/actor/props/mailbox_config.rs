use alloc::boxed::Box;
use core::{
  fmt::{Debug, Formatter, Result as FmtResult},
  num::NonZeroUsize,
};

use fraktor_utils_core_rs::{collections::queue::capabilities::QueueCapabilityRegistry, sync::ArcShared};

use super::{MailboxConfigError, MailboxRequirement};
use crate::dispatch::mailbox::{MailboxFactory, MailboxPolicy, MailboxType, MessagePriorityGenerator, MessageQueue};

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
  /// Returns [`MailboxConfigError::PriorityWithControlAware`] when both a
  /// priority generator and control-aware semantics are requested simultaneously.
  ///
  /// Returns [`MailboxConfigError::PriorityWithDeque`] when both a priority
  /// generator and deque semantics are requested simultaneously.
  ///
  /// Returns [`MailboxConfigError::DequeWithControlAware`] when both control-aware
  /// and deque semantics are requested simultaneously.
  pub fn validate(&self) -> Result<(), MailboxConfigError> {
    if self.stable_priority && self.priority_generator.is_none() {
      return Err(MailboxConfigError::StablePriorityWithoutGenerator);
    }
    if self.priority_generator.is_some() && self.requirement.needs_control_aware() {
      return Err(MailboxConfigError::PriorityWithControlAware);
    }
    if self.priority_generator.is_some() && self.requirement.needs_deque() {
      return Err(MailboxConfigError::PriorityWithDeque);
    }
    if self.requirement.needs_control_aware() && self.requirement.needs_deque() {
      return Err(MailboxConfigError::DequeWithControlAware);
    }
    Ok(())
  }
}

impl Default for MailboxConfig {
  fn default() -> Self {
    MailboxConfig::new(MailboxPolicy::unbounded(None))
  }
}

impl MailboxFactory for MailboxConfig {
  fn mailbox_type(&self) -> ArcShared<dyn MailboxType> {
    // The built-in selection logic returns a `Box<dyn MailboxType>` keyed
    // off policy / requirement / priority_generator / stable_priority.
    // Wrap the resolved factory in `ArcShared` so the caller sees a
    // stable trait-object reference per invocation.
    let boxed = crate::dispatch::mailbox::select_mailbox_type_from_config(self);
    ArcShared::from_boxed(boxed)
  }

  fn create_message_queue(&self) -> Result<Box<dyn MessageQueue>, MailboxConfigError> {
    crate::dispatch::mailbox::create_message_queue_from_config(self)
  }

  fn policy(&self) -> MailboxPolicy {
    self.policy
  }

  fn warn_threshold(&self) -> Option<NonZeroUsize> {
    self.warn_threshold
  }

  fn requirement(&self) -> MailboxRequirement {
    self.requirement
  }

  fn capabilities(&self) -> QueueCapabilityRegistry {
    self.capabilities
  }
}

impl Debug for MailboxConfig {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
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
