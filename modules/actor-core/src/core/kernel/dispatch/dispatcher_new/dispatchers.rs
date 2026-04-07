//! Registry that resolves dispatcher identifiers to configurators.
//!
//! `Dispatchers` is the new dispatcher registry introduced in the
//! dispatcher-pekko-1n-redesign change. It stores configurators behind
//! `ArcShared<Box<dyn MessageDispatcherConfigurator>>` so the entry can be
//! resolved without internal mutability.
//!
//! # Call-frequency contract
//!
//! `Dispatchers::resolve` is intended for spawn / bootstrap paths only. Do not
//! call it from message dispatch hot paths: `PinnedDispatcherConfigurator`
//! constructs a fresh OS thread per call, and unrestricted hot-path resolution
//! would leak threads.

#[cfg(test)]
mod tests;

use alloc::{borrow::ToOwned, boxed::Box, string::String};

use ahash::RandomState;
use fraktor_utils_rs::core::sync::ArcShared;
use hashbrown::{HashMap, hash_map::Entry};

use super::{
  default_dispatcher_configurator::DefaultDispatcherConfigurator, dispatcher_settings::DispatcherSettings,
  dispatchers_error::DispatchersError, executor_shared::ExecutorShared, inline_executor::InlineExecutor,
  message_dispatcher_configurator::MessageDispatcherConfigurator, message_dispatcher_shared::MessageDispatcherShared,
};

/// Reserved registry identifier for the default dispatcher.
pub const DEFAULT_DISPATCHER_ID: &str = "default";
/// Reserved registry identifier for the default blocking IO dispatcher.
pub const DEFAULT_BLOCKING_DISPATCHER_ID: &str = "pekko.actor.default-blocking-io-dispatcher";

const PEKKO_DEFAULT_DISPATCHER_ID: &str = "pekko.actor.default-dispatcher";
const PEKKO_INTERNAL_DISPATCHER_ID: &str = "pekko.actor.internal-dispatcher";

/// Registry mapping dispatcher identifiers to configurators.
pub struct Dispatchers {
  entries: HashMap<String, ArcShared<Box<dyn MessageDispatcherConfigurator>>, RandomState>,
}

impl Clone for Dispatchers {
  fn clone(&self) -> Self {
    Self { entries: self.entries.clone() }
  }
}

impl Default for Dispatchers {
  fn default() -> Self {
    Self::new()
  }
}

impl Dispatchers {
  /// Creates an empty registry.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: HashMap::with_hasher(RandomState::new()) }
  }

  /// Registers a configurator for the supplied identifier.
  ///
  /// # Errors
  ///
  /// Returns [`DispatchersError::Duplicate`] if the identifier already has a registered entry.
  pub fn register(
    &mut self,
    id: impl Into<String>,
    configurator: ArcShared<Box<dyn MessageDispatcherConfigurator>>,
  ) -> Result<(), DispatchersError> {
    let id = id.into();
    match self.entries.entry(id.clone()) {
      | Entry::Occupied(_) => Err(DispatchersError::Duplicate(id)),
      | Entry::Vacant(vacant) => {
        vacant.insert(configurator);
        Ok(())
      },
    }
  }

  /// Registers or replaces the configurator for the supplied identifier.
  pub fn register_or_update(
    &mut self,
    id: impl Into<String>,
    configurator: ArcShared<Box<dyn MessageDispatcherConfigurator>>,
  ) {
    self.entries.insert(id.into(), configurator);
  }

  /// Resolves the [`MessageDispatcherShared`] for the requested identifier.
  ///
  /// **Call-frequency contract**: invoke from spawn / bootstrap paths only.
  /// Hot-path callers must cache the resolved [`MessageDispatcherShared`] (or
  /// the underlying dispatcher handle) instead of calling resolve repeatedly.
  /// `PinnedDispatcherConfigurator` allocates a new OS thread on every call,
  /// so hot-path resolution leaks threads.
  ///
  /// # Errors
  ///
  /// Returns [`DispatchersError::Unknown`] when the identifier has not been
  /// registered.
  pub fn resolve(&self, id: &str) -> Result<MessageDispatcherShared, DispatchersError> {
    let id = Self::normalize_dispatcher_id(id);
    self
      .entries
      .get(id)
      .map(|configurator| configurator.dispatcher())
      .ok_or_else(|| DispatchersError::Unknown(id.to_owned()))
  }

  /// Ensures the default dispatcher entry exists.
  ///
  /// If `default` is missing, the supplied factory closure is called to
  /// produce a configurator that is then registered for both
  /// [`DEFAULT_DISPATCHER_ID`] and [`DEFAULT_BLOCKING_DISPATCHER_ID`].
  pub fn ensure_default(&mut self, factory: impl FnOnce() -> ArcShared<Box<dyn MessageDispatcherConfigurator>>) {
    if !self.entries.contains_key(DEFAULT_DISPATCHER_ID) {
      let configurator = factory();
      self.entries.insert(DEFAULT_DISPATCHER_ID.to_owned(), configurator.clone());
      self.entries.entry(DEFAULT_BLOCKING_DISPATCHER_ID.to_owned()).or_insert(configurator);
    }
  }

  /// Ensures the default dispatcher entry exists, populating it with an
  /// [`InlineExecutor`]-backed [`DefaultDispatcherConfigurator`] when missing.
  ///
  /// This mirrors the legacy `Dispatchers::ensure_default` shape and is the
  /// configuration installed by `ActorSystemConfig::default()` so that all
  /// in-process tests run on the new dispatcher tree without bringing in
  /// `tokio` or another runtime. Production users override the entry through
  /// `ActorSystemConfig::with_new_dispatcher_configurator`.
  pub fn ensure_default_inline(&mut self) {
    self.ensure_default(|| {
      let settings = DispatcherSettings::with_defaults(DEFAULT_DISPATCHER_ID);
      let executor = ExecutorShared::new(InlineExecutor::new());
      let configurator: Box<dyn MessageDispatcherConfigurator> =
        Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
      ArcShared::new(configurator)
    });
  }

  /// Maps a Pekko-style dispatcher identifier to the canonical kernel id.
  #[must_use]
  pub fn normalize_dispatcher_id(id: &str) -> &str {
    match id {
      | PEKKO_DEFAULT_DISPATCHER_ID | PEKKO_INTERNAL_DISPATCHER_ID => DEFAULT_DISPATCHER_ID,
      | _ => id,
    }
  }
}
