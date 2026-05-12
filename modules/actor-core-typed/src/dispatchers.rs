//! Typed dispatcher lookup facade.
//!
//! Corresponds to `org.apache.pekko.actor.typed.Dispatchers` in the Pekko
//! reference implementation. Resolves a [`MessageDispatcherShared`] from a
//! [`DispatcherSelector`] by delegating to the kernel dispatcher registry
//! via [`SystemStateShared`].

#[cfg(test)]
#[path = "dispatchers_test.rs"]
mod tests;

use alloc::string::ToString;

use fraktor_actor_core_kernel_rs::{
  dispatch::dispatcher::{
    DEFAULT_BLOCKING_DISPATCHER_ID as KERNEL_DEFAULT_BLOCKING_DISPATCHER_ID,
    DEFAULT_DISPATCHER_ID as KERNEL_DEFAULT_DISPATCHER_ID, DispatchersError, MessageDispatcherShared,
  },
  system::state::SystemStateShared,
};

use crate::DispatcherSelector;

/// Fraktor public identifier for the default dispatcher.
///
/// Matches [`fraktor_actor_core_kernel_rs::dispatch::dispatcher::DEFAULT_DISPATCHER_ID`]
/// (the kernel primary entry id).
const FRAKTOR_DEFAULT_DISPATCHER_ID: &str = KERNEL_DEFAULT_DISPATCHER_ID;
/// Fraktor public identifier for the internal dispatcher.
const FRAKTOR_INTERNAL_DISPATCHER_ID: &str = "fraktor.actor.internal-dispatcher";

/// Typed facade for looking up dispatcher configurations by selector.
///
/// Corresponds to Pekko's `ActorSystem.dispatchers` / `Dispatchers` abstract
/// class.
///
/// Instances are obtained through
/// [`crate::system::TypedActorSystem::dispatchers`].
#[derive(Clone)]
pub struct Dispatchers {
  state: SystemStateShared,
}

impl Dispatchers {
  /// Well-known identifier for the system default dispatcher.
  ///
  /// Corresponds to the kernel
  /// [`DEFAULT_DISPATCHER_ID`](fraktor_actor_core_kernel_rs::dispatch::dispatcher::DEFAULT_DISPATCHER_ID).
  pub const DEFAULT_DISPATCHER_ID: &str = FRAKTOR_DEFAULT_DISPATCHER_ID;
  /// Well-known identifier for the internal dispatcher.
  ///
  /// Registered by the kernel as an alias of [`Self::DEFAULT_DISPATCHER_ID`].
  pub const INTERNAL_DISPATCHER_ID: &str = FRAKTOR_INTERNAL_DISPATCHER_ID;

  /// Creates a new dispatcher lookup facade.
  #[must_use]
  pub(crate) const fn new(state: SystemStateShared) -> Self {
    Self { state }
  }

  /// Resolves a dispatcher handle for the given selector.
  ///
  /// # Selector mapping
  ///
  /// | Selector | Passed to kernel `resolve` |
  /// |----------|----------------------------|
  /// | `Default` / `SameAsParent` | [`Self::DEFAULT_DISPATCHER_ID`] (`"fraktor.actor.default-dispatcher"`) |
  /// | `FromConfig(id)` | `id` verbatim (kernel follows its alias chain) |
  /// | `Blocking` | kernel `DEFAULT_BLOCKING_DISPATCHER_ID` (`"fraktor.actor.default-blocking-io-dispatcher"`) |
  ///
  /// The `FromConfig` arm passes the identifier through unchanged so that
  /// kernel [`Dispatchers`](fraktor_actor_core_kernel_rs::dispatch::dispatcher::Dispatchers)
  /// alias chain resolution is authoritative. This preserves any
  /// user-provided entry override such as
  /// `register_or_update("fraktor.actor.default-dispatcher", custom)`, which
  /// a stale typed-level normalization step would otherwise shadow.
  ///
  /// # Errors
  ///
  /// Returns [`DispatchersError::Unknown`] when the resolved identifier
  /// has not been registered in the kernel dispatcher registry.
  pub fn lookup(&self, selector: &DispatcherSelector) -> Result<MessageDispatcherShared, DispatchersError> {
    let id: &str = match selector {
      | DispatcherSelector::Default | DispatcherSelector::SameAsParent => Self::DEFAULT_DISPATCHER_ID,
      | DispatcherSelector::FromConfig(id) => id,
      | DispatcherSelector::Blocking => KERNEL_DEFAULT_BLOCKING_DISPATCHER_ID,
    };
    self.state.resolve_dispatcher(id).ok_or_else(|| DispatchersError::Unknown(id.to_string()))
  }

  /// Shuts down the typed dispatcher facade.
  ///
  /// This is intentionally a no-op because dispatcher runtime ownership stays
  /// with the actor system and underlying executor.
  pub const fn shutdown(&self) {}
}
