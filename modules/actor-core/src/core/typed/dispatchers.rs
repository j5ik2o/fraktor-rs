//! Typed dispatcher lookup facade.
//!
//! Corresponds to `org.apache.pekko.actor.typed.Dispatchers` in the Pekko
//! reference implementation. Resolves a [`MessageDispatcherShared`] from a
//! [`DispatcherSelector`] by delegating to the kernel dispatcher registry
//! via [`SystemStateShared`].

#[cfg(test)]
mod tests;

use crate::core::{
  kernel::{
    dispatch::dispatcher::{DEFAULT_BLOCKING_DISPATCHER_ID, DispatchersError, MessageDispatcherShared},
    system::state::SystemStateShared,
  },
  typed::DispatcherSelector,
};

/// Internal registry id for the default dispatcher entry.
const REGISTERED_DEFAULT_DISPATCHER_ID: &str = "default";
/// Pekko-compatible public identifier for the default dispatcher.
const PEKKO_DEFAULT_DISPATCHER_ID: &str = "pekko.actor.default-dispatcher";
/// Pekko-compatible public identifier for the internal dispatcher.
const PEKKO_INTERNAL_DISPATCHER_ID: &str = "pekko.actor.internal-dispatcher";

/// Typed facade for looking up dispatcher configurations by selector.
///
/// Corresponds to Pekko's `ActorSystem.dispatchers` / `Dispatchers` abstract
/// class.
///
/// Instances are obtained through
/// [`crate::core::typed::system::TypedActorSystem::dispatchers`].
#[derive(Clone)]
pub struct Dispatchers {
  state: SystemStateShared,
}

impl Dispatchers {
  /// Well-known identifier for the system default dispatcher.
  ///
  /// Corresponds to Pekko's `Dispatchers.DefaultDispatcherId`.
  pub const DEFAULT_DISPATCHER_ID: &str = PEKKO_DEFAULT_DISPATCHER_ID;
  /// Well-known identifier for the internal dispatcher.
  ///
  /// Corresponds to Pekko's `Dispatchers.InternalDispatcherId`.
  pub const INTERNAL_DISPATCHER_ID: &str = PEKKO_INTERNAL_DISPATCHER_ID;

  /// Creates a new dispatcher lookup facade.
  #[must_use]
  pub(crate) const fn new(state: SystemStateShared) -> Self {
    Self { state }
  }

  /// Resolves a dispatcher handle for the given selector.
  ///
  /// # Selector mapping
  ///
  /// | Selector | Resolved id |
  /// |----------|-------------|
  /// | `Default` | `"default"` |
  /// | `FromConfig("pekko.actor.default-dispatcher")` | `"default"` |
  /// | `FromConfig(id)` | the provided `id` |
  /// | `SameAsParent` | `"default"` (parent inheritance is handled at spawn time) |
  /// | `Blocking` | `"pekko.actor.default-blocking-io-dispatcher"` |
  ///
  /// # Errors
  ///
  /// Returns [`DispatchersError::Unknown`] when the resolved identifier
  /// has not been registered in the kernel dispatcher registry.
  pub fn lookup(&self, selector: &DispatcherSelector) -> Result<MessageDispatcherShared, DispatchersError> {
    let id = match selector {
      | DispatcherSelector::Default | DispatcherSelector::SameAsParent => REGISTERED_DEFAULT_DISPATCHER_ID,
      | DispatcherSelector::FromConfig(id) => Self::normalize_dispatcher_id(id),
      | DispatcherSelector::Blocking => DEFAULT_BLOCKING_DISPATCHER_ID,
    };
    self.state.resolve_dispatcher(id).ok_or_else(|| DispatchersError::Unknown(alloc::string::ToString::to_string(id)))
  }

  /// Shuts down the typed dispatcher facade.
  ///
  /// This is intentionally a no-op because dispatcher runtime ownership stays
  /// with the actor system and underlying executor.
  pub const fn shutdown(&self) {}

  fn normalize_dispatcher_id(id: &str) -> &str {
    match id {
      | Self::DEFAULT_DISPATCHER_ID | Self::INTERNAL_DISPATCHER_ID => REGISTERED_DEFAULT_DISPATCHER_ID,
      | _ => id,
    }
  }
}
