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
use core::sync::atomic::{AtomicUsize, Ordering};

use ahash::RandomState;
use fraktor_utils_core_rs::core::sync::ArcShared;
use hashbrown::{HashMap, hash_map::Entry};

use super::{
  default_dispatcher_configurator::DefaultDispatcherConfigurator, dispatcher_config::DispatcherConfig,
  dispatchers_error::DispatchersError, executor_shared::ExecutorShared, inline_executor::InlineExecutor,
  message_dispatcher_configurator::MessageDispatcherConfigurator, message_dispatcher_shared::MessageDispatcherShared,
  trampoline_state::TrampolineState,
};

/// Reserved registry identifier for the default dispatcher.
pub const DEFAULT_DISPATCHER_ID: &str = "default";
/// Reserved registry identifier for the default blocking IO dispatcher.
pub const DEFAULT_BLOCKING_DISPATCHER_ID: &str = "pekko.actor.default-blocking-io-dispatcher";

const PEKKO_DEFAULT_DISPATCHER_ID: &str = "pekko.actor.default-dispatcher";
const PEKKO_INTERNAL_DISPATCHER_ID: &str = "pekko.actor.internal-dispatcher";

/// Registry mapping dispatcher identifiers to configurators.
pub struct Dispatchers {
  entries:       HashMap<String, ArcShared<Box<dyn MessageDispatcherConfigurator>>, RandomState>,
  /// Cumulative `resolve()` invocation counter.
  ///
  /// Wrapped in `ArcShared<AtomicUsize>` so that all clones of a single
  /// `Dispatchers` instance share the same counter. This is the runtime
  /// observation surface for the call-frequency contract documented on
  /// [`Dispatchers::resolve`]: tests and diagnostics can read the counter
  /// before/after a workload to verify that hot-path callers are not
  /// invoking `resolve` outside the spawn / bootstrap window.
  resolve_count: ArcShared<AtomicUsize>,
}

impl Clone for Dispatchers {
  fn clone(&self) -> Self {
    Self { entries: self.entries.clone(), resolve_count: self.resolve_count.clone() }
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
    Self { entries: HashMap::with_hasher(RandomState::new()), resolve_count: ArcShared::new(AtomicUsize::new(0)) }
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
  /// Each invocation increments the diagnostic counter exposed by
  /// [`Dispatchers::resolve_call_count`], regardless of whether the lookup
  /// succeeded.
  ///
  /// # Errors
  ///
  /// Returns [`DispatchersError::Unknown`] when the identifier has not been
  /// registered.
  pub fn resolve(&self, id: &str) -> Result<MessageDispatcherShared, DispatchersError> {
    self.resolve_count.fetch_add(1, Ordering::Relaxed);
    let id = Self::normalize_dispatcher_id(id);
    self
      .entries
      .get(id)
      .map(|configurator| configurator.dispatcher())
      .ok_or_else(|| DispatchersError::Unknown(id.to_owned()))
  }

  /// Returns the cumulative number of [`Dispatchers::resolve`] invocations
  /// observed by this registry instance and all of its clones.
  ///
  /// Diagnostics-only accessor used by integration tests and benches to
  /// verify the call-frequency contract: `resolve` should be called from
  /// spawn / bootstrap paths only, never from message hot paths. Read the
  /// counter before and after a representative workload and assert that
  /// the message-only portion of the workload does not change the value.
  #[must_use]
  pub fn resolve_call_count(&self) -> usize {
    self.resolve_count.load(Ordering::Relaxed)
  }

  fn build_default_inline_configurator() -> ArcShared<Box<dyn MessageDispatcherConfigurator>> {
    let settings = DispatcherConfig::with_defaults(DEFAULT_DISPATCHER_ID);
    let executor = ExecutorShared::new(Box::new(InlineExecutor::new()), TrampolineState::new());
    let configurator: Box<dyn MessageDispatcherConfigurator> =
      Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
    ArcShared::new(configurator)
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
  /// `ActorSystemConfig::with_dispatcher_configurator`.
  pub fn ensure_default_inline(&mut self) {
    self.ensure_default(Self::build_default_inline_configurator);
  }

  /// Replaces the seeded default inline dispatcher.
  ///
  /// When the default blocking dispatcher still aliases the same configurator as
  /// `default`, it is updated to keep both reserved ids on the same provider.
  /// Explicit blocking-dispatcher overrides are preserved.
  pub fn replace_default_inline(&mut self) {
    let replace_blocking = self
      .entries
      .get(DEFAULT_BLOCKING_DISPATCHER_ID)
      .zip(self.entries.get(DEFAULT_DISPATCHER_ID))
      .is_some_and(|(blocking, default)| ArcShared::ptr_eq(blocking, default));
    let configurator = Self::build_default_inline_configurator();
    self.entries.insert(DEFAULT_DISPATCHER_ID.to_owned(), configurator.clone());
    if replace_blocking || !self.entries.contains_key(DEFAULT_BLOCKING_DISPATCHER_ID) {
      self.entries.insert(DEFAULT_BLOCKING_DISPATCHER_ID.to_owned(), configurator);
    }
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
