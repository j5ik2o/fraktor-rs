//! Builder-facing factory trait that bundles a [`MailboxType`] with the
//! metadata consulted by the actor-cell spawn path.

#[cfg(test)]
#[path = "mailbox_factory_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::num::NonZeroUsize;

use fraktor_utils_core_rs::{collections::queue::capabilities::QueueCapabilityRegistry, sync::ArcShared};

use super::{MailboxPolicy, MailboxType, MessageQueue};
use crate::actor::props::{MailboxConfigError, MailboxRequirement};

/// Builder-facing extension point for mailbox installation.
///
/// `MailboxFactory` is the high-level factory registered with
/// [`ActorSystemConfig::with_mailbox`](crate::actor::setup::ActorSystemConfig::with_mailbox).
/// It bundles a low-level [`MailboxType`] (which produces the concrete
/// [`MessageQueue`]) with the metadata consulted by `ActorCell` at spawn
/// time (policy, warn threshold, required and available queue capabilities).
///
/// Unlike [`MailboxType`], which is intentionally a thin queue-factory trait,
/// `MailboxFactory` is the unit users plug into the builder. Custom
/// implementations can produce arbitrary mailbox behavior while advertising
/// the metadata the runtime needs for instrumentation and capability checks.
///
/// [`MailboxConfig`](crate::actor::props::MailboxConfig)
/// implements this trait as a bridge so existing callers keep working
/// unchanged.
pub trait MailboxFactory: Send + Sync {
  /// Returns the underlying [`MailboxType`].
  ///
  /// The returned factory is invoked during mailbox construction to produce
  /// the concrete [`MessageQueue`]. Implementations should return a stable
  /// `ArcShared` so each call resolves to the same factory instance.
  fn mailbox_type(&self) -> ArcShared<dyn MailboxType>;

  /// Creates a user-message queue.
  ///
  /// The default implementation delegates to
  /// [`mailbox_type().create()`](MailboxType::create). Factory implementations
  /// that need richer construction (e.g. configuration validation) can
  /// override this.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxConfigError`] when the factory rejects the
  /// configuration it was built with.
  fn create_message_queue(&self) -> Result<Box<dyn MessageQueue>, MailboxConfigError> {
    Ok(self.mailbox_type().create())
  }

  /// Returns the policy used for instrumentation (capacity / throughput).
  ///
  /// The default value is unbounded with no throughput cap. Custom factories
  /// can override to feed capacity metrics and throughput limits into the
  /// runtime instrumentation layer.
  fn policy(&self) -> MailboxPolicy {
    MailboxPolicy::unbounded(None)
  }

  /// Returns the warning threshold for queue depth instrumentation.
  ///
  /// When `Some(n)`, the mailbox instrumentation layer emits a warning
  /// once the queue depth exceeds `n`. The default is `None` (no warning).
  fn warn_threshold(&self) -> Option<NonZeroUsize> {
    None
  }

  /// Returns the queue capabilities this factory **requires**.
  ///
  /// `ActorCell::create` validates that the `capabilities()` registry
  /// provides every capability listed here before constructing the mailbox.
  fn requirement(&self) -> MailboxRequirement {
    MailboxRequirement::none()
  }

  /// Returns the queue capabilities this factory **advertises**.
  ///
  /// Used in conjunction with [`requirement`](Self::requirement) for
  /// spawn-time capability checks.
  fn capabilities(&self) -> QueueCapabilityRegistry {
    QueueCapabilityRegistry::with_defaults()
  }
}
