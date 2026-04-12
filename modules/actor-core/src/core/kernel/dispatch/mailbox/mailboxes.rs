use alloc::{borrow::ToOwned, boxed::Box, string::String};
use core::{marker::PhantomData, num::NonZeroUsize};

use ahash::RandomState;
use fraktor_utils_core_rs::core::sync::ArcShared;
use hashbrown::HashMap;

use crate::core::kernel::{
  actor::props::{MailboxConfig, MailboxConfigError},
  dispatch::mailbox::{
    BoundedPriorityMessageQueueStateSharedFactory, BoundedStablePriorityMessageQueueStateSharedFactory,
    MailboxRegistryError, bounded_mailbox_type::BoundedMailboxType,
    bounded_priority_mailbox_type::BoundedPriorityMailboxType,
    bounded_stable_priority_mailbox_type::BoundedStablePriorityMailboxType, capacity::MailboxCapacity,
    mailbox_type::MailboxType, message_priority_generator::MessagePriorityGenerator, message_queue::MessageQueue,
    overflow_strategy::MailboxOverflowStrategy, policy::MailboxPolicy,
    unbounded_control_aware_mailbox_type::UnboundedControlAwareMailboxType,
    unbounded_deque_mailbox_type::UnboundedDequeMailboxType, unbounded_mailbox_type::UnboundedMailboxType,
    unbounded_priority_mailbox_type::UnboundedPriorityMailboxType,
    unbounded_stable_priority_mailbox_type::UnboundedStablePriorityMailboxType,
  },
  system::shared_factory::BuiltinSpinSharedFactory,
};

#[cfg(test)]
mod tests;

const DEFAULT_MAILBOX_ID: &str = "default";

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
  bounded_stable_priority_state_shared_factory: &ArcShared<dyn BoundedStablePriorityMessageQueueStateSharedFactory>,
) -> Result<Box<dyn MessageQueue>, MailboxConfigError> {
  config.validate()?;
  if let Some(generator) = config.priority_generator() {
    if config.stable_priority() {
      return Ok(
        stable_priority_mailbox_type_from_config(
          generator.clone(),
          bounded_stable_priority_state_shared_factory,
          config.policy(),
        )
        .create(),
      );
    }
    return Ok(priority_mailbox_type_from_config(generator.clone(), config.policy()).create());
  }
  if config.requirement().needs_control_aware() {
    let mailbox_type: Box<dyn MailboxType> = Box::new(UnboundedControlAwareMailboxType::new());
    return Ok(mailbox_type.create());
  }
  if config.requirement().needs_deque() {
    return Ok(deque_mailbox_type_from_policy(config.policy())?.create());
  }
  Ok(create_message_queue_from_policy(config.policy()))
}

fn priority_mailbox_type_from_config(
  generator: ArcShared<dyn MessagePriorityGenerator>,
  policy: MailboxPolicy,
) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => {
      let state_shared_factory: ArcShared<dyn BoundedPriorityMessageQueueStateSharedFactory> =
        ArcShared::new(BuiltinSpinSharedFactory::new());
      Box::new(BoundedPriorityMailboxType::new(generator, state_shared_factory, capacity, policy.overflow()))
    },
    | MailboxCapacity::Unbounded => Box::new(UnboundedPriorityMailboxType::new(generator)),
  }
}

fn stable_priority_mailbox_type_from_config(
  generator: ArcShared<dyn MessagePriorityGenerator>,
  state_shared_factory: &ArcShared<dyn BoundedStablePriorityMessageQueueStateSharedFactory>,
  policy: MailboxPolicy,
) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => Box::new(BoundedStablePriorityMailboxType::new(
      generator,
      state_shared_factory.clone(),
      capacity,
      policy.overflow(),
    )),
    | MailboxCapacity::Unbounded => Box::new(UnboundedStablePriorityMailboxType::new(generator)),
  }
}

fn deque_mailbox_type_from_policy(policy: MailboxPolicy) -> Result<Box<dyn MailboxType>, MailboxConfigError> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { .. } => Err(MailboxConfigError::BoundedWithDeque),
    | MailboxCapacity::Unbounded => Ok(Box::new(UnboundedDequeMailboxType::new())),
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

/// Registry that manages mailbox configurations keyed by identifier.
pub struct Mailboxes {
  entries: HashMap<String, MailboxConfig, RandomState>,
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

  /// Registers a mailbox configuration.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Duplicate`] when the identifier already exists.
  pub fn register(&mut self, id: impl Into<String>, config: MailboxConfig) -> Result<(), MailboxRegistryError> {
    let id = id.into();
    if self.entries.contains_key(&id) {
      return Err(MailboxRegistryError::duplicate(&id));
    }
    self.entries.insert(id, config);
    Ok(())
  }

  /// Registers or updates a mailbox configuration for the provided identifier.
  ///
  /// If the identifier already exists, the configuration is updated.
  pub fn register_or_update(&mut self, id: impl Into<String>, config: MailboxConfig) {
    self.entries.insert(id.into(), config);
  }

  /// Resolves the mailbox configuration for the provided identifier.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve(&self, id: &str) -> Result<MailboxConfig, MailboxRegistryError> {
    self.entries.get(id).cloned().ok_or_else(|| MailboxRegistryError::unknown(id))
  }

  /// Creates a user-message queue from the configuration registered under `id`.
  ///
  /// When a priority generator is present, a priority-based queue is produced.
  /// When the registered configuration declares deque semantics and the policy is
  /// unbounded, this returns a deque-capable queue.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn create_message_queue(
    &self,
    id: &str,
    bounded_stable_priority_state_shared_factory: &ArcShared<dyn BoundedStablePriorityMessageQueueStateSharedFactory>,
  ) -> Result<Box<dyn MessageQueue>, MailboxRegistryError> {
    let config = self.resolve(id)?;
    Ok(create_message_queue_from_config(&config, bounded_stable_priority_state_shared_factory)?)
  }

  /// Ensures the default mailbox configuration is registered.
  pub fn ensure_default(&mut self) {
    self.entries.entry(DEFAULT_MAILBOX_ID.to_owned()).or_default();
  }
}

impl Default for Mailboxes {
  fn default() -> Self {
    Self::new()
  }
}
