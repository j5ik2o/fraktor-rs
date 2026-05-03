use alloc::{borrow::ToOwned, boxed::Box, string::String};
use core::{marker::PhantomData, num::NonZeroUsize};

use ahash::RandomState;
use fraktor_utils_core_rs::core::sync::ArcShared;
use hashbrown::HashMap;

use crate::core::kernel::{
  actor::props::{MailboxConfig, MailboxConfigError},
  dispatch::mailbox::{
    MailboxFactory, MailboxRegistryError, bounded_control_aware_mailbox_type::BoundedControlAwareMailboxType,
    bounded_deque_mailbox_type::BoundedDequeMailboxType, bounded_mailbox_type::BoundedMailboxType,
    bounded_priority_mailbox_type::BoundedPriorityMailboxType,
    bounded_stable_priority_mailbox_type::BoundedStablePriorityMailboxType, capacity::MailboxCapacity,
    mailbox_type::MailboxType, message_priority_generator::MessagePriorityGenerator, message_queue::MessageQueue,
    overflow_strategy::MailboxOverflowStrategy, policy::MailboxPolicy,
    unbounded_control_aware_mailbox_type::UnboundedControlAwareMailboxType,
    unbounded_deque_mailbox_type::UnboundedDequeMailboxType, unbounded_mailbox_type::UnboundedMailboxType,
    unbounded_priority_mailbox_type::UnboundedPriorityMailboxType,
    unbounded_stable_priority_mailbox_type::UnboundedStablePriorityMailboxType,
  },
};

#[cfg(test)]
mod tests;

/// Primary registry identifier for the default mailbox entry.
///
/// Corresponds 1:1 to Pekko `Mailboxes.DefaultMailboxId` in
/// `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Mailboxes.scala:58`.
///
/// Historically fraktor-rs used `"default"` as the primary id, but change
/// `pekko-dispatcher-primary-id-alignment` (2026-04-23) flipped the value
/// to match Pekko. `Mailboxes` does not currently support alias chain
/// resolution (unlike `Dispatchers`), so the legacy `"default"` token is
/// simply no longer registered.
const DEFAULT_MAILBOX_ID: &str = "faktor.actor.default-mailbox";

pub(crate) fn create_message_queue_from_policy(policy: MailboxPolicy) -> Box<dyn MessageQueue> {
  mailbox_type_from_policy(policy).create()
}

/// Creates a message queue considering the policy, requirement, and priority generator
/// from the config.
///
/// When a priority generator is present, a priority-based queue is returned regardless
/// of other requirements. When the config declares deque semantics and the policy is
/// unbounded, this returns an [`UnboundedDequeMessageQueue`].
///
/// # Errors
///
/// Returns [`MailboxConfigError`] when the configuration contract is violated
/// (e.g. `stable_priority` enabled without a priority generator).
pub(crate) fn create_message_queue_from_config(
  config: &MailboxConfig,
) -> Result<Box<dyn MessageQueue>, MailboxConfigError> {
  config.validate()?;
  Ok(select_mailbox_type_from_config(config).create())
}

/// Selects the [`MailboxType`] that matches the supplied [`MailboxConfig`].
///
/// Selection precedence:
/// 1. `priority_generator` with `stable_priority` → stable-priority factory
/// 2. `priority_generator` → priority factory
/// 3. `requirement.needs_control_aware()` → control-aware factory
/// 4. `requirement.needs_deque()` → deque factory
/// 5. default → capacity-based unbounded / bounded factory
///
/// Callers that invoke [`MailboxType::create`] directly on the returned
/// factory must pre-validate via
/// [`MailboxConfig::validate`](crate::core::kernel::actor::props::MailboxConfig::validate).
pub(crate) fn select_mailbox_type_from_config(config: &MailboxConfig) -> Box<dyn MailboxType> {
  if let Some(generator) = config.priority_generator() {
    if config.stable_priority() {
      return stable_priority_mailbox_type_from_config(generator.clone(), config.policy());
    }
    return priority_mailbox_type_from_config(generator.clone(), config.policy());
  }
  if config.requirement().needs_control_aware() {
    return control_aware_mailbox_type_from_policy(config.policy());
  }
  if config.requirement().needs_deque() {
    return deque_mailbox_type_from_policy(config.policy());
  }
  mailbox_type_from_policy(config.policy())
}

fn priority_mailbox_type_from_config(
  generator: ArcShared<dyn MessagePriorityGenerator>,
  policy: MailboxPolicy,
) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => {
      Box::new(BoundedPriorityMailboxType::new(generator, capacity, policy.overflow()))
    },
    | MailboxCapacity::Unbounded => Box::new(UnboundedPriorityMailboxType::new(generator)),
  }
}

fn stable_priority_mailbox_type_from_config(
  generator: ArcShared<dyn MessagePriorityGenerator>,
  policy: MailboxPolicy,
) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => {
      Box::new(BoundedStablePriorityMailboxType::new(generator, capacity, policy.overflow()))
    },
    | MailboxCapacity::Unbounded => Box::new(UnboundedStablePriorityMailboxType::new(generator)),
  }
}

fn deque_mailbox_type_from_policy(policy: MailboxPolicy) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => Box::new(BoundedDequeMailboxType::new(capacity, policy.overflow())),
    | MailboxCapacity::Unbounded => Box::new(UnboundedDequeMailboxType::new()),
  }
}

fn control_aware_mailbox_type_from_policy(policy: MailboxPolicy) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => {
      Box::new(BoundedControlAwareMailboxType::new(capacity, policy.overflow()))
    },
    | MailboxCapacity::Unbounded => Box::new(UnboundedControlAwareMailboxType::new()),
  }
}

fn mailbox_type_from_policy(policy: MailboxPolicy) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => bounded_mailbox_type(capacity, policy.overflow()),
    | MailboxCapacity::Unbounded => Box::new(UnboundedMailboxType::new()),
  }
}

fn bounded_mailbox_type(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy) -> Box<dyn MailboxType> {
  Box::new(BoundedMailboxType::new(capacity, overflow))
}

/// Registry that manages mailbox factories keyed by identifier.
///
/// Each entry is stored as a trait-object factory
/// ([`ArcShared<dyn MailboxFactory>`]). [`MailboxConfig`] implements
/// [`MailboxFactory`] as a bridge so high-level callers register a
/// `MailboxConfig` and low-level callers pass a custom
/// [`MailboxFactory`] implementation directly.
pub struct Mailboxes {
  entries: HashMap<String, ArcShared<dyn MailboxFactory>, RandomState>,
  _marker: PhantomData<()>,
}

impl Clone for Mailboxes {
  fn clone(&self) -> Self {
    Self { entries: self.entries.clone(), _marker: PhantomData }
  }
}

impl Mailboxes {
  /// Creates an empty mailbox registry.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: HashMap::with_hasher(RandomState::new()), _marker: PhantomData }
  }

  /// Registers a mailbox factory.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Duplicate`] when the identifier already exists.
  pub fn register(
    &mut self,
    id: impl Into<String>,
    factory: impl MailboxFactory + 'static,
  ) -> Result<(), MailboxRegistryError> {
    let id = id.into();
    if self.entries.contains_key(&id) {
      return Err(MailboxRegistryError::duplicate(&id));
    }
    self.entries.insert(id, ArcShared::new(factory));
    Ok(())
  }

  /// Registers or updates a mailbox factory for the provided identifier.
  ///
  /// If the identifier already exists, the factory is replaced.
  pub fn register_or_update(&mut self, id: impl Into<String>, factory: impl MailboxFactory + 'static) {
    self.entries.insert(id.into(), ArcShared::new(factory));
  }

  /// Resolves the mailbox factory for the provided identifier.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve(&self, id: &str) -> Result<ArcShared<dyn MailboxFactory>, MailboxRegistryError> {
    self.entries.get(id).cloned().ok_or_else(|| MailboxRegistryError::unknown(id))
  }

  /// Creates a user-message queue from the factory registered under `id`.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been
  /// registered; wraps [`MailboxConfigError`] from the factory via
  /// [`MailboxRegistryError::from`].
  pub fn create_message_queue(&self, id: &str) -> Result<Box<dyn MessageQueue>, MailboxRegistryError> {
    let factory = self.resolve(id)?;
    Ok(factory.create_message_queue()?)
  }

  /// Ensures the default mailbox configuration is registered.
  pub fn ensure_default(&mut self) {
    self.entries.entry(DEFAULT_MAILBOX_ID.to_owned()).or_insert_with(|| ArcShared::new(MailboxConfig::default()));
  }
}

impl Default for Mailboxes {
  fn default() -> Self {
    Self::new()
  }
}
