use alloc::{borrow::ToOwned, boxed::Box, format, string::String};
use core::{marker::PhantomData, num::NonZeroUsize, time::Duration};

use ahash::RandomState;
use fraktor_utils_core_rs::sync::ArcShared;
use hashbrown::HashMap;

use crate::{
  actor::props::{MailboxConfig, MailboxConfigError, MailboxRequirement},
  dispatch::mailbox::{
    MailboxFactory, MailboxRegistryError, MailboxSelection,
    bounded_control_aware_mailbox_type::BoundedControlAwareMailboxType,
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
#[path = "mailboxes_test.rs"]
mod tests;

/// Primary registry identifier for the default mailbox entry.
///
/// The actual value is `"fraktor.actor.default-mailbox"`. This uses the
/// Fraktor namespace and intentionally differs from Pekko
/// `Mailboxes.DefaultMailboxId` (`"pekko.actor.default-mailbox"`).
///
/// Historically fraktor-rs used `"default"` as the primary id. `Mailboxes`
/// does not currently support alias chain resolution (unlike `Dispatchers`),
/// so the legacy `"default"` token is no longer registered after the Fraktor
/// namespace split.
const DEFAULT_MAILBOX_ID: &str = "fraktor.actor.default-mailbox";

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
/// [`MailboxConfig::validate`](crate::actor::props::MailboxConfig::validate).
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
  if config.requirement().needs_multiple_consumer() {
    return multiple_consumer_mailbox_type_from_policy(config.policy());
  }
  mailbox_type_from_policy(config.policy())
}

fn priority_mailbox_type_from_config(
  generator: ArcShared<dyn MessagePriorityGenerator>,
  policy: MailboxPolicy,
) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => match policy.push_timeout() {
      | Some(push_timeout) => Box::new(BoundedPriorityMailboxType::new_with_push_timeout(
        generator,
        capacity,
        policy.overflow(),
        push_timeout,
      )),
      | None => Box::new(BoundedPriorityMailboxType::new(generator, capacity, policy.overflow())),
    },
    | MailboxCapacity::Unbounded => Box::new(UnboundedPriorityMailboxType::new(generator)),
  }
}

fn stable_priority_mailbox_type_from_config(
  generator: ArcShared<dyn MessagePriorityGenerator>,
  policy: MailboxPolicy,
) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => match policy.push_timeout() {
      | Some(push_timeout) => Box::new(BoundedStablePriorityMailboxType::new_with_push_timeout(
        generator,
        capacity,
        policy.overflow(),
        push_timeout,
      )),
      | None => Box::new(BoundedStablePriorityMailboxType::new(generator, capacity, policy.overflow())),
    },
    | MailboxCapacity::Unbounded => Box::new(UnboundedStablePriorityMailboxType::new(generator)),
  }
}

fn deque_mailbox_type_from_policy(policy: MailboxPolicy) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => match policy.push_timeout() {
      | Some(push_timeout) => {
        Box::new(BoundedDequeMailboxType::new_with_push_timeout(capacity, policy.overflow(), push_timeout))
      },
      | None => Box::new(BoundedDequeMailboxType::new(capacity, policy.overflow())),
    },
    | MailboxCapacity::Unbounded => Box::new(UnboundedDequeMailboxType::new()),
  }
}

fn control_aware_mailbox_type_from_policy(policy: MailboxPolicy) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => match policy.push_timeout() {
      | Some(push_timeout) => {
        Box::new(BoundedControlAwareMailboxType::new_with_push_timeout(capacity, policy.overflow(), push_timeout))
      },
      | None => Box::new(BoundedControlAwareMailboxType::new(capacity, policy.overflow())),
    },
    | MailboxCapacity::Unbounded => Box::new(UnboundedControlAwareMailboxType::new()),
  }
}

fn multiple_consumer_mailbox_type_from_policy(policy: MailboxPolicy) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => bounded_mailbox_type(capacity, policy.overflow(), policy.push_timeout()),
    | MailboxCapacity::Unbounded => Box::new(UnboundedDequeMailboxType::new()),
  }
}

fn mailbox_type_from_policy(policy: MailboxPolicy) -> Box<dyn MailboxType> {
  match policy.capacity() {
    | MailboxCapacity::Bounded { capacity } => bounded_mailbox_type(capacity, policy.overflow(), policy.push_timeout()),
    | MailboxCapacity::Unbounded => Box::new(UnboundedMailboxType::new()),
  }
}

fn bounded_mailbox_type(
  capacity: NonZeroUsize,
  overflow: MailboxOverflowStrategy,
  push_timeout: Option<Duration>,
) -> Box<dyn MailboxType> {
  match push_timeout {
    | Some(push_timeout) => Box::new(BoundedMailboxType::new_with_push_timeout(capacity, overflow, push_timeout)),
    | None => Box::new(BoundedMailboxType::new(capacity, overflow)),
  }
}

/// Registry that manages mailbox factories keyed by identifier.
///
/// Each entry is stored as a trait-object factory
/// ([`ArcShared<dyn MailboxFactory>`]). [`MailboxConfig`] implements
/// [`MailboxFactory`] as a bridge so high-level callers register a
/// `MailboxConfig` and low-level callers pass a custom
/// [`MailboxFactory`] implementation directly.
pub struct Mailboxes {
  entries:             HashMap<String, ArcShared<dyn MailboxFactory>, RandomState>,
  queue_type_bindings: HashMap<MailboxRequirement, String, RandomState>,
  _marker:             PhantomData<()>,
}

impl Clone for Mailboxes {
  fn clone(&self) -> Self {
    Self {
      entries:             self.entries.clone(),
      queue_type_bindings: self.queue_type_bindings.clone(),
      _marker:             PhantomData,
    }
  }
}

impl Mailboxes {
  /// Creates an empty mailbox registry.
  #[must_use]
  pub fn new() -> Self {
    Self {
      entries:             HashMap::with_hasher(RandomState::new()),
      queue_type_bindings: HashMap::with_hasher(RandomState::new()),
      _marker:             PhantomData,
    }
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

  /// Binds a queue requirement to a mailbox identifier.
  ///
  /// This mirrors Pekko's `pekko.actor.mailbox.requirements` mapping used by
  /// `Mailboxes.lookupByQueueType`.
  pub fn bind_queue_type(&mut self, requirement: MailboxRequirement, id: impl Into<String>) {
    self.queue_type_bindings.insert(requirement, id.into());
  }

  /// Resolves the mailbox factory for the provided identifier.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve(&self, id: &str) -> Result<ArcShared<dyn MailboxFactory>, MailboxRegistryError> {
    self.entries.get(id).cloned().ok_or_else(|| MailboxRegistryError::unknown(id))
  }

  /// Resolves the mailbox factory bound to `requirement`.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when no binding or mailbox
  /// factory exists for the requested requirement.
  pub fn lookup_by_queue_type(
    &self,
    requirement: MailboxRequirement,
  ) -> Result<ArcShared<dyn MailboxFactory>, MailboxRegistryError> {
    let id = self
      .queue_type_bindings
      .get(&requirement)
      .ok_or_else(|| MailboxRegistryError::unknown(format!("{requirement:?}")))?;
    self.resolve(id)
  }

  /// Selects a mailbox factory using Pekko-style precedence.
  ///
  /// Selection order is explicit mailbox id, dispatcher mailbox id, actor
  /// queue requirement, dispatcher queue requirement, and default mailbox.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError`] when the selected id or queue-type
  /// binding cannot be resolved.
  pub fn select(&self, selection: &MailboxSelection) -> Result<ArcShared<dyn MailboxFactory>, MailboxRegistryError> {
    if let Some(id) = selection.explicit_mailbox_id() {
      return self.resolve(id);
    }
    if let Some(id) = selection.dispatcher_mailbox_id() {
      return self.resolve(id);
    }
    if !selection.actor_requirement().is_empty() {
      match self.lookup_by_queue_type(selection.actor_requirement()) {
        | Ok(factory) => return Ok(factory),
        | Err(_) if !selection.dispatcher_requirement().is_empty() => {},
        | Err(error) => return Err(error),
      }
    }
    if !selection.dispatcher_requirement().is_empty() {
      return self.lookup_by_queue_type(selection.dispatcher_requirement());
    }
    self.resolve(DEFAULT_MAILBOX_ID)
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
    self.bind_queue_type(MailboxRequirement::none(), DEFAULT_MAILBOX_ID);
  }
}

impl Default for Mailboxes {
  fn default() -> Self {
    Self::new()
  }
}
