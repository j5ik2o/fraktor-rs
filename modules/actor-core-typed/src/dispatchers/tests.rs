use fraktor_actor_core_kernel_rs::dispatch::dispatcher::DispatchersError;

use crate::{DispatcherSelector, dispatchers::Dispatchers};

// --- helpers ---------------------------------------------------------------

fn new_dispatchers_with_defaults() -> Dispatchers {
  let system = fraktor_actor_adaptor_std_rs::system::new_noop_actor_system();
  Dispatchers::new(system.state())
}

// --- lookup: Default selector ----------------------------------------------

#[test]
fn lookup_default_selector_resolves_default_dispatcher() {
  // Given: a Dispatchers facade backed by a system with default dispatchers
  let dispatchers = new_dispatchers_with_defaults();

  // When: lookup is called with DispatcherSelector::Default
  let result = dispatchers.lookup(&DispatcherSelector::Default);

  // Then: a valid dispatcher handle is returned
  assert!(result.is_ok(), "Default selector should resolve to the default dispatcher");
}

// --- lookup: FromConfig selector -------------------------------------------

#[test]
fn lookup_from_config_selector_resolves_registered_dispatcher() {
  let dispatchers = new_dispatchers_with_defaults();

  let selector = DispatcherSelector::from_config(Dispatchers::DEFAULT_DISPATCHER_ID);
  let result = dispatchers.lookup(&selector);

  assert!(result.is_ok(), "FromConfig(DefaultDispatcherId) should resolve to the default dispatcher");
}

#[test]
fn lookup_from_config_selector_resolves_internal_dispatcher_id_via_kernel_alias() {
  // 旧実装では typed 層で normalize_dispatcher_id が Pekko id を `"default"` に
  // 書き換えていたが、本テストは kernel の alias chain 経由で等価に解決されることを確認する
  // (`ensure_default_inline` が `pekko.actor.internal-dispatcher → default` の alias を登録する)。
  let dispatchers = new_dispatchers_with_defaults();

  let selector = DispatcherSelector::from_config(Dispatchers::INTERNAL_DISPATCHER_ID);
  let result = dispatchers.lookup(&selector);

  assert!(result.is_ok(), "FromConfig(InternalDispatcherId) should resolve via kernel alias chain");
}

#[test]
fn lookup_from_config_preserves_user_override_of_pekko_alias() {
  // Bugbot Medium 回帰防止: 旧実装では typed 層の normalize_dispatcher_id が Pekko id を
  // 常に `"default"` にマッピングしていたため、`register_or_update("pekko.actor.default-dispatcher",
  // custom)` によるユーザー上書きを typed 経路が bypass していた。本テストは kernel alias chain
  // (alias は register_or_update 実行時に wipe される) を typed facade が尊重することを確認する。
  use alloc::boxed::Box;
  use core::time::Duration;

  use fraktor_actor_core_kernel_rs::dispatch::dispatcher::{
    DefaultDispatcherFactory, DispatcherConfig, ExecuteError, Executor, ExecutorShared, MessageDispatcherFactory,
    TrampolineState,
  };
  use fraktor_utils_core_rs::sync::ArcShared;

  struct NoopExecutor;
  impl Executor for NoopExecutor {
    fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
      Ok(())
    }

    fn shutdown(&mut self) {}
  }

  // custom configurator を Pekko id の entry として register_or_update 経由で登録する。
  // 既存の `ensure_default_inline` が登録した alias は register_or_update の wipe により除去される。
  let system = fraktor_actor_adaptor_std_rs::system::new_noop_actor_system_with(|config| {
    let custom_config =
      DispatcherConfig::with_defaults("custom-typed-dispatcher").with_shutdown_timeout(Duration::from_secs(2));
    let executor = ExecutorShared::new(Box::new(NoopExecutor), TrampolineState::new());
    let custom: ArcShared<Box<dyn MessageDispatcherFactory>> =
      ArcShared::new(Box::new(DefaultDispatcherFactory::new(&custom_config, executor)));
    config.with_dispatcher_factory("fraktor.actor.default-dispatcher", custom)
  });

  let dispatchers = Dispatchers::new(system.state());
  let selector = DispatcherSelector::from_config(Dispatchers::DEFAULT_DISPATCHER_ID);
  let resolved = dispatchers.lookup(&selector).expect("resolve via typed facade");
  assert_eq!(
    resolved.id(),
    "custom-typed-dispatcher",
    "typed facade must honour kernel alias / entry resolution; user override must not be shadowed"
  );
}

#[test]
fn lookup_from_config_selector_returns_error_for_unknown_id() {
  let dispatchers = new_dispatchers_with_defaults();

  let selector = DispatcherSelector::from_config("non-existent-dispatcher");
  let result = dispatchers.lookup(&selector);

  assert!(
    matches!(result, Err(DispatchersError::Unknown(_))),
    "Unknown dispatcher id should return DispatchersError::Unknown"
  );
}

// --- lookup: SameAsParent selector -----------------------------------------

#[test]
fn lookup_same_as_parent_selector_falls_back_to_default() {
  let dispatchers = new_dispatchers_with_defaults();

  let result = dispatchers.lookup(&DispatcherSelector::SameAsParent);

  assert!(result.is_ok(), "SameAsParent should fall back to the default dispatcher in lookup");
}

// --- lookup: Blocking selector ---------------------------------------------

#[test]
fn lookup_blocking_selector_resolves_blocking_dispatcher() {
  let dispatchers = new_dispatchers_with_defaults();

  let result = dispatchers.lookup(&DispatcherSelector::Blocking);

  assert!(result.is_ok(), "Blocking selector should resolve to the blocking dispatcher");
}

// --- DEFAULT_DISPATCHER_ID constant ----------------------------------------

#[test]
fn default_dispatcher_id_matches_kernel_constant() {
  let id = Dispatchers::DEFAULT_DISPATCHER_ID;
  assert_eq!(id, "fraktor.actor.default-dispatcher");
}

#[test]
fn internal_dispatcher_id_matches_pekko_constant() {
  let id = Dispatchers::INTERNAL_DISPATCHER_ID;
  assert_eq!(id, "fraktor.actor.internal-dispatcher");
}

// --- shutdown --------------------------------------------------------------

#[test]
fn shutdown_is_callable_as_a_noop() {
  let dispatchers = new_dispatchers_with_defaults();

  dispatchers.shutdown();

  let result = dispatchers.lookup(&DispatcherSelector::Default);
  assert!(result.is_ok(), "shutdown() should not invalidate dispatcher lookup");
}

// --- lookup consistency: Default and SameAsParent resolve to same handle ----

#[test]
fn lookup_default_and_same_as_parent_resolve_to_equivalent_handle() {
  let dispatchers = new_dispatchers_with_defaults();

  let default_handle = dispatchers.lookup(&DispatcherSelector::Default).expect("Default");
  let same_as_parent_handle = dispatchers.lookup(&DispatcherSelector::SameAsParent).expect("SameAsParent");

  // Both selectors should target the same registered dispatcher id, so the
  // resolved handles report the same identifier and throughput.
  assert_eq!(default_handle.id(), same_as_parent_handle.id());
  assert_eq!(default_handle.throughput(), same_as_parent_handle.throughput());
}
