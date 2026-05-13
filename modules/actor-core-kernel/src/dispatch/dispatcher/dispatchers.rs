//! Registry that resolves dispatcher identifiers to configurators.
//!
//! `Dispatchers` is the new dispatcher registry introduced in the
//! dispatcher-pekko-1n-redesign change. It stores configurators behind
//! `ArcShared<Box<dyn MessageDispatcherFactory>>` so the entry can be
//! resolved without internal mutability.
//!
//! # Alias chain resolution
//!
//! Identifiers can be registered either as concrete entries
//! (`register` / `register_or_update`) or as aliases that redirect to another
//! identifier (`register_alias`). Aliases are followed up to
//! [`Dispatchers::MAX_ALIAS_DEPTH`] levels before resolution fails with
//! [`DispatchersError::AliasChainTooDeep`]. Mirrors Pekko
//! `Dispatchers.lookupConfigurator` (`Dispatchers.scala:159-198`) and
//! `MaxDispatcherAliasDepth = 20` (`Dispatchers.scala:146`).
//!
//! # Call-frequency contract
//!
//! `Dispatchers::resolve` is intended for spawn / bootstrap paths only. Do not
//! call it from message dispatch hot paths: `PinnedDispatcherFactory`
//! constructs a fresh OS thread per call, and unrestricted hot-path resolution
//! would leak threads.

#[cfg(test)]
#[path = "dispatchers_test.rs"]
mod tests;

use alloc::{borrow::ToOwned, boxed::Box, string::String};
use core::sync::atomic::{AtomicUsize, Ordering};

use ahash::RandomState;
use fraktor_utils_core_rs::sync::ArcShared;
use hashbrown::{HashMap, hash_map::Entry};

use super::{
  default_dispatcher_factory::DefaultDispatcherFactory, dispatcher_config::DispatcherConfig,
  dispatchers_error::DispatchersError, executor_shared::ExecutorShared, inline_executor::InlineExecutor,
  message_dispatcher_factory::MessageDispatcherFactory, message_dispatcher_shared::MessageDispatcherShared,
  trampoline_state::TrampolineState,
};

/// Primary registry identifier for the default dispatcher entry.
///
/// The actual value is `"fraktor.actor.default-dispatcher"`. This uses the
/// Fraktor namespace and no longer matches Pekko
/// `Dispatchers.DefaultDispatcherId` (`"pekko.actor.default-dispatcher"`).
/// Internal code should use [`DEFAULT_DISPATCHER_ID`]; external configuration
/// should use the Fraktor literal when a literal is required.
///
/// Historically fraktor-rs used `"default"` as the primary id. The legacy
/// `"default"` token is no longer registered as an entry or alias.
pub const DEFAULT_DISPATCHER_ID: &str = "fraktor.actor.default-dispatcher";
/// Reserved registry identifier for the default blocking IO dispatcher.
///
/// The actual value is `"fraktor.actor.default-blocking-io-dispatcher"`.
/// This uses the Fraktor namespace and no longer matches Pekko
/// `Dispatchers.DefaultBlockingDispatcherId`.
pub const DEFAULT_BLOCKING_DISPATCHER_ID: &str = "fraktor.actor.default-blocking-io-dispatcher";

/// Fraktor-style alias for the internal dispatcher.
///
/// Registered as an alias of [`DEFAULT_DISPATCHER_ID`] by `ensure_default` so
/// internal dispatcher references resolve to the same entry.
const FRAKTOR_INTERNAL_DISPATCHER_ID: &str = "fraktor.actor.internal-dispatcher";

/// Registry mapping dispatcher identifiers to configurators.
pub struct Dispatchers {
  entries:       HashMap<String, ArcShared<Box<dyn MessageDispatcherFactory>>, RandomState>,
  /// Alias identifiers redirecting to another id (target may itself be an alias).
  ///
  /// Kept separate from `entries` so that `register` and `register_alias` can
  /// detect cross-map conflicts at registration time rather than at resolve
  /// time.
  aliases:       HashMap<String, String, RandomState>,
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
    Self {
      entries:       self.entries.clone(),
      aliases:       self.aliases.clone(),
      resolve_count: self.resolve_count.clone(),
    }
  }
}

impl Default for Dispatchers {
  fn default() -> Self {
    Self::new()
  }
}

impl Dispatchers {
  /// Maximum alias chain depth before rejection.
  ///
  /// Matches Pekko `Dispatchers.MaxDispatcherAliasDepth = 20`
  /// (`Dispatchers.scala:146`).
  pub const MAX_ALIAS_DEPTH: usize = 20;

  /// Creates an empty registry.
  #[must_use]
  pub fn new() -> Self {
    Self {
      entries:       HashMap::with_hasher(RandomState::new()),
      aliases:       HashMap::with_hasher(RandomState::new()),
      resolve_count: ArcShared::new(AtomicUsize::new(0)),
    }
  }

  /// Registers a configurator for the supplied identifier.
  ///
  /// # Errors
  ///
  /// - [`DispatchersError::AliasConflictsWithEntry`] if the identifier is already registered as an
  ///   alias.
  /// - [`DispatchersError::Duplicate`] if the identifier already has a registered entry.
  pub fn register(
    &mut self,
    id: impl Into<String>,
    configurator: ArcShared<Box<dyn MessageDispatcherFactory>>,
  ) -> Result<(), DispatchersError> {
    let id = id.into();
    if self.aliases.contains_key(&id) {
      return Err(DispatchersError::AliasConflictsWithEntry(id));
    }
    match self.entries.entry(id.clone()) {
      | Entry::Occupied(_) => Err(DispatchersError::Duplicate(id)),
      | Entry::Vacant(vacant) => {
        vacant.insert(configurator);
        Ok(())
      },
    }
  }

  /// Registers or replaces the configurator for the supplied identifier.
  ///
  /// Last-writer-wins semantics: any existing entry is replaced, and any
  /// existing alias with the same identifier is removed so the new entry
  /// takes precedence. This keeps the method infallible so it composes
  /// cleanly with builder-style configuration (e.g.
  /// `ActorSystemConfig::with_dispatcher_factory`).
  pub fn register_or_update(
    &mut self,
    id: impl Into<String>,
    configurator: ArcShared<Box<dyn MessageDispatcherFactory>>,
  ) {
    let id = id.into();
    self.aliases.remove(&id);
    self.entries.insert(id, configurator);
  }

  /// Registers an alias identifier that redirects to another dispatcher id.
  ///
  /// The `target` id may itself be an alias (chains are supported up to
  /// [`Self::MAX_ALIAS_DEPTH`] levels) and does not need to be registered at
  /// the time this method is called; alias targets are validated lazily on
  /// `resolve`.
  ///
  /// # Errors
  ///
  /// - [`DispatchersError::AliasConflictsWithEntry`] if `alias` is already registered as a concrete
  ///   entry.
  /// - [`DispatchersError::Duplicate`] if `alias` already has an alias registration.
  pub fn register_alias(
    &mut self,
    alias: impl Into<String>,
    target: impl Into<String>,
  ) -> Result<(), DispatchersError> {
    let alias = alias.into();
    if self.entries.contains_key(&alias) {
      return Err(DispatchersError::AliasConflictsWithEntry(alias));
    }
    match self.aliases.entry(alias.clone()) {
      | Entry::Occupied(_) => Err(DispatchersError::Duplicate(alias)),
      | Entry::Vacant(vacant) => {
        vacant.insert(target.into());
        Ok(())
      },
    }
  }

  /// Resolves the [`MessageDispatcherShared`] for the requested identifier.
  ///
  /// Follows the alias chain (up to [`Self::MAX_ALIAS_DEPTH`] levels) before
  /// looking up the final identifier in the entry map.
  ///
  /// **Call-frequency contract**: invoke from spawn / bootstrap paths only.
  /// Hot-path callers must cache the resolved [`MessageDispatcherShared`] (or
  /// the underlying dispatcher handle) instead of calling resolve repeatedly.
  /// `PinnedDispatcherFactory` allocates a new OS thread on every call,
  /// so hot-path resolution leaks threads.
  ///
  /// Each invocation increments the diagnostic counter exposed by
  /// [`Dispatchers::resolve_call_count`] exactly once, regardless of the
  /// alias chain depth or whether the lookup ultimately succeeds.
  ///
  /// # Errors
  ///
  /// - [`DispatchersError::AliasChainTooDeep`] when the alias chain exceeds
  ///   [`Self::MAX_ALIAS_DEPTH`].
  /// - [`DispatchersError::Unknown`] when the final (non-alias) identifier is not registered.
  pub fn resolve(&self, id: &str) -> Result<MessageDispatcherShared, DispatchersError> {
    self.resolve_count.fetch_add(1, Ordering::Relaxed);
    let resolved = self.follow_alias_chain(id)?;
    self.entries.get(&resolved).map(|configurator| configurator.dispatcher()).ok_or(DispatchersError::Unknown(resolved))
  }

  /// Returns the canonical (fully-alias-resolved) identifier for `id`.
  ///
  /// Follows the alias chain the same way as [`Self::resolve`] and verifies
  /// that the final identifier is registered as a concrete entry. Intended
  /// for callers that need to record the canonical id (e.g. to tie an actor
  /// cell to its dispatcher for later diagnostics) without constructing a
  /// [`MessageDispatcherShared`].
  ///
  /// Does **not** increment [`Self::resolve_call_count`].
  ///
  /// # Errors
  ///
  /// - [`DispatchersError::AliasChainTooDeep`] when the alias chain exceeds
  ///   [`Self::MAX_ALIAS_DEPTH`].
  /// - [`DispatchersError::Unknown`] when the final identifier is not registered as a concrete
  ///   entry.
  pub fn canonical_id(&self, id: &str) -> Result<String, DispatchersError> {
    let resolved = self.follow_alias_chain(id)?;
    if self.entries.contains_key(&resolved) { Ok(resolved) } else { Err(DispatchersError::Unknown(resolved)) }
  }

  /// Follows the alias chain from `id` and returns the final (non-alias)
  /// identifier.
  ///
  /// Returns `Ok(id.to_owned())` immediately when `id` is not an alias.
  /// Returns [`DispatchersError::AliasChainTooDeep`] when the chain exceeds
  /// [`Self::MAX_ALIAS_DEPTH`]. Cycles are detected implicitly through the
  /// depth limit (matching Pekko `Dispatchers.scala:160-163`).
  fn follow_alias_chain(&self, id: &str) -> Result<String, DispatchersError> {
    let mut current = id.to_owned();
    // Pekko の `if (depth > MaxDispatcherAliasDepth)` ガードに合わせ、MAX_ALIAS_DEPTH 回までの
    // alias hop を許容し、(MAX_ALIAS_DEPTH + 1) 回目で error を返す。
    for _ in 0..=Self::MAX_ALIAS_DEPTH {
      match self.aliases.get(&current) {
        | Some(target) => current = target.clone(),
        | None => return Ok(current),
      }
    }
    Err(DispatchersError::AliasChainTooDeep { start: id.to_owned(), depth: Self::MAX_ALIAS_DEPTH })
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

  fn build_default_inline_configurator() -> ArcShared<Box<dyn MessageDispatcherFactory>> {
    let settings = DispatcherConfig::with_defaults(DEFAULT_DISPATCHER_ID);
    let executor = ExecutorShared::new(Box::new(InlineExecutor::new()), TrampolineState::new());
    let configurator: Box<dyn MessageDispatcherFactory> = Box::new(DefaultDispatcherFactory::new(&settings, executor));
    ArcShared::new(configurator)
  }

  /// Registers the Fraktor internal dispatcher alias pointing at
  /// [`DEFAULT_DISPATCHER_ID`].
  ///
  /// Only `fraktor.actor.internal-dispatcher` is registered as an alias;
  /// `fraktor.actor.default-dispatcher` is the primary entry id itself after
  /// change `pekko-dispatcher-primary-id-alignment` (2026-04-23), so it must
  /// not be registered as an alias. The legacy fraktor-rs `"default"` token
  /// is also not registered (completely retired by the same change).
  ///
  /// Idempotent: if the alias is already registered or the id has been
  /// explicitly overridden as a concrete entry (e.g. by
  /// `register_or_update`), the call is a no-op.
  fn register_internal_dispatcher_alias(&mut self) {
    Self::register_alias_if_absent(
      &mut self.aliases,
      &self.entries,
      FRAKTOR_INTERNAL_DISPATCHER_ID,
      DEFAULT_DISPATCHER_ID,
    );
  }

  /// Inserts an alias pointing at `target` if `alias` is neither a
  /// registered entry nor a registered alias. Idempotent helper that
  /// bypasses the `register_alias` Result API (we own both maps
  /// exclusively, so the checks here are equivalent).
  fn register_alias_if_absent(
    aliases: &mut HashMap<String, String, RandomState>,
    entries: &HashMap<String, ArcShared<Box<dyn MessageDispatcherFactory>>, RandomState>,
    alias: &str,
    target: &'static str,
  ) {
    if entries.contains_key(alias) {
      return;
    }
    aliases.entry(alias.to_owned()).or_insert_with(|| target.to_owned());
  }

  /// Ensures the default dispatcher entry exists.
  ///
  /// If `default` is missing, the supplied factory closure is called to
  /// produce a configurator that is then registered for both
  /// [`DEFAULT_DISPATCHER_ID`] and [`DEFAULT_BLOCKING_DISPATCHER_ID`], and the
  /// Fraktor internal dispatcher alias is registered against
  /// [`DEFAULT_DISPATCHER_ID`].
  ///
  /// Any pre-existing alias registered under the same id is removed before
  /// entry insertion (matching [`Self::register_or_update`] semantics) so
  /// the freshly inserted entry is reachable via `resolve` without being
  /// shadowed by a stale alias.
  pub fn ensure_default(&mut self, factory: impl FnOnce() -> ArcShared<Box<dyn MessageDispatcherFactory>>) {
    if !self.entries.contains_key(DEFAULT_DISPATCHER_ID) {
      let configurator = factory();
      self.insert_entry_wiping_alias(DEFAULT_DISPATCHER_ID, configurator.clone());
      if !self.entries.contains_key(DEFAULT_BLOCKING_DISPATCHER_ID) {
        self.insert_entry_wiping_alias(DEFAULT_BLOCKING_DISPATCHER_ID, configurator);
      }
    }
    self.register_internal_dispatcher_alias();
  }

  /// Ensures the default dispatcher entry exists, populating it with an
  /// [`InlineExecutor`]-backed [`DefaultDispatcherFactory`] when missing.
  ///
  /// This mirrors the legacy `Dispatchers::ensure_default` shape and is the
  /// configuration installed by `ActorSystemConfig::default()` so that all
  /// in-process tests run on the new dispatcher tree without bringing in
  /// `tokio` or another runtime. Production users override the entry through
  /// `ActorSystemConfig::with_dispatcher_factory`.
  pub fn ensure_default_inline(&mut self) {
    self.ensure_default(Self::build_default_inline_configurator);
  }

  /// Replaces the seeded default inline dispatcher.
  ///
  /// When the default blocking dispatcher still aliases the same configurator as
  /// `default`, it is updated to keep both reserved ids on the same provider.
  /// Explicit blocking-dispatcher overrides are preserved.
  ///
  /// Any pre-existing alias registered under [`DEFAULT_DISPATCHER_ID`] (or
  /// [`DEFAULT_BLOCKING_DISPATCHER_ID`] when the blocking alias is being
  /// rebuilt) is removed before entry insertion so the freshly inserted
  /// entry is reachable via `resolve` without being shadowed by a stale
  /// alias.
  pub fn replace_default_inline(&mut self) {
    let replace_blocking = self
      .entries
      .get(DEFAULT_BLOCKING_DISPATCHER_ID)
      .zip(self.entries.get(DEFAULT_DISPATCHER_ID))
      .is_some_and(|(blocking, default)| ArcShared::ptr_eq(blocking, default));
    let configurator = Self::build_default_inline_configurator();
    self.insert_entry_wiping_alias(DEFAULT_DISPATCHER_ID, configurator.clone());
    if replace_blocking || !self.entries.contains_key(DEFAULT_BLOCKING_DISPATCHER_ID) {
      self.insert_entry_wiping_alias(DEFAULT_BLOCKING_DISPATCHER_ID, configurator);
    }
    self.register_internal_dispatcher_alias();
  }

  /// Inserts a configurator into `entries` after removing any alias for the
  /// same id. Factored out so `ensure_default` / `replace_default_inline`
  /// stay consistent with [`Self::register_or_update`] last-writer-wins
  /// semantics and a pre-existing alias cannot shadow the fresh entry via
  /// [`Self::follow_alias_chain`].
  fn insert_entry_wiping_alias(&mut self, id: &str, configurator: ArcShared<Box<dyn MessageDispatcherFactory>>) {
    self.aliases.remove(id);
    self.entries.insert(id.to_owned(), configurator);
  }
}
