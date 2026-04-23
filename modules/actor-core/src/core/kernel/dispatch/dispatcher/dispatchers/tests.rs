use alloc::boxed::Box;
use core::time::Duration;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{DEFAULT_BLOCKING_DISPATCHER_ID, DEFAULT_DISPATCHER_ID, Dispatchers, DispatchersError};
use crate::core::kernel::dispatch::dispatcher::{
  DefaultDispatcherConfigurator, DispatcherConfig, ExecuteError, Executor, ExecutorShared,
  MessageDispatcherConfigurator, TrampolineState,
};

struct NoopExecutor;

impl Executor for NoopExecutor {
  fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    Ok(())
  }

  fn shutdown(&mut self) {}
}

fn make_default_configurator(id: &str) -> ArcShared<Box<dyn MessageDispatcherConfigurator>> {
  let settings = DispatcherConfig::with_defaults(id).with_shutdown_timeout(Duration::from_secs(2));
  let executor = ExecutorShared::new(Box::new(NoopExecutor), TrampolineState::new());
  let configurator: Box<dyn MessageDispatcherConfigurator> =
    Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
  ArcShared::new(configurator)
}

#[test]
fn register_then_resolve_returns_same_dispatcher() {
  let mut dispatchers = Dispatchers::new();
  let configurator = make_default_configurator("default");
  dispatchers.register("default", configurator).expect("register");
  let shared = dispatchers.resolve("default").expect("resolve");
  assert_eq!(shared.id(), "default");
}

#[test]
fn duplicate_register_returns_error() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register("dup", make_default_configurator("dup")).expect("first");
  match dispatchers.register("dup", make_default_configurator("dup")) {
    | Ok(()) => panic!("expected duplicate error"),
    | Err(err) => assert!(matches!(err, DispatchersError::Duplicate(_))),
  }
}

#[test]
fn unknown_resolve_returns_error() {
  let dispatchers = Dispatchers::new();
  match dispatchers.resolve("missing") {
    | Ok(_) => panic!("expected unknown id error"),
    | Err(err) => assert!(matches!(err, DispatchersError::Unknown(_))),
  }
}

#[test]
fn pekko_default_dispatcher_id_resolves_via_alias_registered_by_ensure_default() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default(|| make_default_configurator("default"));
  let resolved = dispatchers.resolve("pekko.actor.default-dispatcher").expect("resolve compat id");
  assert_eq!(resolved.id(), "default");
}

#[test]
fn pekko_internal_dispatcher_id_resolves_via_alias_registered_by_ensure_default() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default(|| make_default_configurator("default"));
  let resolved = dispatchers.resolve("pekko.actor.internal-dispatcher").expect("resolve internal");
  assert_eq!(resolved.id(), "default");
}

#[test]
fn ensure_default_inserts_when_missing() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default(|| make_default_configurator("default"));
  let resolved = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve default");
  assert_eq!(resolved.id(), "default");
  let blocking = dispatchers.resolve(DEFAULT_BLOCKING_DISPATCHER_ID).expect("resolve blocking");
  assert_eq!(blocking.id(), "default");
}

#[test]
fn ensure_default_is_idempotent_when_present() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register(DEFAULT_DISPATCHER_ID, make_default_configurator("first")).expect("register");
  dispatchers.ensure_default(|| make_default_configurator("second"));
  // The original configurator stays.
  let resolved = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve default");
  assert_eq!(resolved.id(), "first");
}

#[test]
fn replace_default_inline_updates_seeded_default_aliases() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default_inline();
  let seeded_default = dispatchers.entries.get(DEFAULT_DISPATCHER_ID).expect("seeded default").clone();
  let seeded_blocking = dispatchers.entries.get(DEFAULT_BLOCKING_DISPATCHER_ID).expect("seeded blocking").clone();
  assert!(
    ArcShared::ptr_eq(&seeded_default, &seeded_blocking),
    "seeded default/blocking dispatchers should share the same configurator"
  );

  dispatchers.replace_default_inline();

  let replaced_default = dispatchers.entries.get(DEFAULT_DISPATCHER_ID).expect("replaced default");
  let replaced_blocking = dispatchers.entries.get(DEFAULT_BLOCKING_DISPATCHER_ID).expect("replaced blocking");
  assert!(
    !ArcShared::ptr_eq(&seeded_default, replaced_default),
    "default dispatcher should be rebuilt when the lock provider changes"
  );
  assert!(
    ArcShared::ptr_eq(replaced_default, replaced_blocking),
    "seeded blocking alias should follow the rebuilt default dispatcher"
  );
}

#[test]
fn replace_default_inline_preserves_custom_blocking_dispatcher() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default_inline();
  let seeded_default = dispatchers.entries.get(DEFAULT_DISPATCHER_ID).expect("seeded default").clone();
  let custom_blocking = make_default_configurator("blocking");
  dispatchers.register_or_update(DEFAULT_BLOCKING_DISPATCHER_ID, custom_blocking.clone());

  dispatchers.replace_default_inline();

  let replaced_default = dispatchers.entries.get(DEFAULT_DISPATCHER_ID).expect("replaced default");
  let blocking = dispatchers.entries.get(DEFAULT_BLOCKING_DISPATCHER_ID).expect("blocking");
  assert!(
    !ArcShared::ptr_eq(&seeded_default, replaced_default),
    "default dispatcher should still be rebuilt when blocking is overridden"
  );
  assert!(
    ArcShared::ptr_eq(blocking, &custom_blocking),
    "custom blocking dispatcher must not be overwritten by lock-provider replacement"
  );
}

#[test]
fn resolve_call_count_starts_at_zero_and_increments_per_call() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register("default", make_default_configurator("default")).expect("register");
  assert_eq!(dispatchers.resolve_call_count(), 0);
  let _ = dispatchers.resolve("default").expect("resolve 1");
  assert_eq!(dispatchers.resolve_call_count(), 1);
  let _ = dispatchers.resolve("default").expect("resolve 2");
  let _ = dispatchers.resolve("default").expect("resolve 3");
  assert_eq!(dispatchers.resolve_call_count(), 3);
}

#[test]
fn resolve_call_count_increments_even_on_unknown_id() {
  let dispatchers = Dispatchers::new();
  assert_eq!(dispatchers.resolve_call_count(), 0);
  let _ = dispatchers.resolve("missing");
  let _ = dispatchers.resolve("missing");
  // Failed lookups still bump the counter so the diagnostic captures the
  // full call traffic into the registry, not just successful resolutions.
  assert_eq!(dispatchers.resolve_call_count(), 2);
}

#[test]
fn resolve_call_count_is_shared_across_clones() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register("default", make_default_configurator("default")).expect("register");
  let cloned = dispatchers.clone();
  let _ = dispatchers.resolve("default").expect("resolve from original");
  let _ = cloned.resolve("default").expect("resolve from clone");
  // Clones share the same counter so the diagnostic accurately reflects the
  // total call traffic regardless of which Dispatchers handle observed it.
  assert_eq!(dispatchers.resolve_call_count(), 2);
  assert_eq!(cloned.resolve_call_count(), 2);
}

// --- Alias chain tests (change `pekko-dispatcher-alias-chain`, spec Scenarios 1-9) ---

#[test]
fn single_hop_alias_resolves_to_target_entry() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register("default", make_default_configurator("default")).expect("register default");
  dispatchers.register_alias("app.work", "default").expect("register alias");

  let before = dispatchers.resolve_call_count();
  let resolved = dispatchers.resolve("app.work").expect("resolve alias");
  assert_eq!(resolved.id(), "default");
  assert_eq!(dispatchers.resolve_call_count(), before + 1, "resolve() must bump the counter exactly once per call");
}

#[test]
fn multi_hop_alias_chain_resolves_to_terminal_entry() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register("A", make_default_configurator("A")).expect("register A");
  dispatchers.register_alias("B", "A").expect("alias B->A");
  dispatchers.register_alias("C", "B").expect("alias C->B");
  dispatchers.register_alias("D", "C").expect("alias D->C");

  let resolved = dispatchers.resolve("D").expect("resolve D");
  assert_eq!(resolved.id(), "A");
}

#[test]
fn alias_chain_exceeding_max_depth_returns_alias_chain_too_deep() {
  let mut dispatchers = Dispatchers::new();
  // Build a strictly-linear chain of (MAX_ALIAS_DEPTH + 1) aliases; the
  // final id is never registered as an entry. resolving the head should
  // report depth-exceeded rather than Unknown.
  for step in 0..=Dispatchers::MAX_ALIAS_DEPTH {
    let alias = alloc::format!("alias_{step}");
    let target = alloc::format!("alias_{}", step + 1);
    dispatchers.register_alias(alias, target).expect("alias chain step");
  }

  match dispatchers.resolve("alias_0") {
    | Ok(_) => panic!("expected AliasChainTooDeep"),
    | Err(DispatchersError::AliasChainTooDeep { start, depth }) => {
      assert_eq!(start, "alias_0");
      assert_eq!(depth, Dispatchers::MAX_ALIAS_DEPTH);
    },
    | Err(other) => panic!("expected AliasChainTooDeep, got {other:?}"),
  }
}

#[test]
fn alias_cycle_is_detected_as_alias_chain_too_deep() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register_alias("A", "B").expect("alias A->B");
  dispatchers.register_alias("B", "A").expect("alias B->A");

  match dispatchers.resolve("A") {
    | Ok(_) => panic!("expected AliasChainTooDeep for cycle"),
    | Err(DispatchersError::AliasChainTooDeep { start, depth }) => {
      assert_eq!(start, "A");
      assert_eq!(depth, Dispatchers::MAX_ALIAS_DEPTH);
    },
    | Err(other) => panic!("expected AliasChainTooDeep for cycle, got {other:?}"),
  }
}

#[test]
fn alias_to_missing_target_surfaces_unknown_on_terminal_id() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register_alias("work", "missing-dispatcher").expect("register alias");

  match dispatchers.resolve("work") {
    | Ok(_) => panic!("expected Unknown(missing-dispatcher)"),
    | Err(DispatchersError::Unknown(id)) => assert_eq!(id, "missing-dispatcher"),
    | Err(other) => panic!("expected Unknown(missing-dispatcher), got {other:?}"),
  }
}

#[test]
fn register_rejects_id_already_registered_as_alias() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register("default", make_default_configurator("default")).expect("register default");
  dispatchers.register_alias("foo", "default").expect("register alias");

  match dispatchers.register("foo", make_default_configurator("foo")) {
    | Err(DispatchersError::AliasConflictsWithEntry(id)) => assert_eq!(id, "foo"),
    | other => panic!("expected AliasConflictsWithEntry, got {other:?}"),
  }

  // The alias entry must still be intact and still resolve to `default`.
  let resolved = dispatchers.resolve("foo").expect("resolve");
  assert_eq!(resolved.id(), "default");
}

#[test]
fn register_alias_rejects_id_already_registered_as_entry() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register("foo", make_default_configurator("foo")).expect("register foo");

  match dispatchers.register_alias("foo", "default") {
    | Err(DispatchersError::AliasConflictsWithEntry(id)) => assert_eq!(id, "foo"),
    | other => panic!("expected AliasConflictsWithEntry, got {other:?}"),
  }

  // The entry must remain untouched.
  let resolved = dispatchers.resolve("foo").expect("resolve foo");
  assert_eq!(resolved.id(), "foo");
}

#[test]
fn register_alias_rejects_duplicate_alias() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register_alias("foo", "default").expect("first alias");

  match dispatchers.register_alias("foo", "other") {
    | Err(DispatchersError::Duplicate(id)) => assert_eq!(id, "foo"),
    | other => panic!("expected Duplicate, got {other:?}"),
  }

  // The original alias target must be preserved.
  assert_eq!(dispatchers.aliases.get("foo").map(String::as_str), Some("default"));
}

#[test]
fn register_or_update_is_lenient_and_wipes_existing_alias() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default(|| make_default_configurator("default"));
  // Confirm pekko alias is in place.
  let via_alias = dispatchers.resolve("pekko.actor.default-dispatcher").expect("resolve via alias");
  assert_eq!(via_alias.id(), "default");

  // Replace the alias with a concrete entry using register_or_update (builder
  // path). This must succeed unconditionally (no Result) and wipe the alias.
  let custom = make_default_configurator("custom-pekko-default");
  dispatchers.register_or_update("pekko.actor.default-dispatcher", custom);

  let resolved = dispatchers.resolve("pekko.actor.default-dispatcher").expect("resolve after override");
  assert_eq!(resolved.id(), "custom-pekko-default");

  // The `default` entry (previous alias target) must remain untouched.
  let still_default = dispatchers.resolve("default").expect("resolve default");
  assert_eq!(still_default.id(), "default");
}

#[test]
fn canonical_id_returns_resolved_entry_id() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default(|| make_default_configurator("default"));

  let canonical = dispatchers.canonical_id("pekko.actor.default-dispatcher").expect("canonical");
  assert_eq!(canonical, "default");

  // canonical_id must NOT bump the resolve counter.
  assert_eq!(dispatchers.resolve_call_count(), 0);
}

#[test]
fn canonical_id_returns_unknown_when_terminal_is_missing() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register_alias("work", "missing").expect("register alias");

  match dispatchers.canonical_id("work") {
    | Err(DispatchersError::Unknown(id)) => assert_eq!(id, "missing"),
    | other => panic!("expected Unknown, got {other:?}"),
  }
}

#[test]
fn ensure_default_wipes_preexisting_alias_for_default_id() {
  // Simulate the Bugbot-reported scenario: a caller registers an alias under
  // `DEFAULT_DISPATCHER_ID` first and then calls `ensure_default`. Without
  // the wipe, the alias would shadow the freshly inserted entry because
  // `follow_alias_chain` consults `aliases` before `entries`.
  let mut dispatchers = Dispatchers::new();
  dispatchers.register_alias(DEFAULT_DISPATCHER_ID, "some-other-id").expect("pre-existing alias");

  dispatchers.ensure_default(|| make_default_configurator("default"));

  let resolved = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve default after wipe");
  assert_eq!(resolved.id(), "default");
  assert!(
    !dispatchers.aliases.contains_key(DEFAULT_DISPATCHER_ID),
    "alias for DEFAULT_DISPATCHER_ID must be wiped on ensure_default"
  );
}

#[test]
fn replace_default_inline_wipes_preexisting_alias_for_default_id() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register_alias(DEFAULT_DISPATCHER_ID, "some-other-id").expect("pre-existing alias");

  dispatchers.replace_default_inline();

  let resolved = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve default after replace");
  assert_eq!(resolved.id(), "default");
  assert!(
    !dispatchers.aliases.contains_key(DEFAULT_DISPATCHER_ID),
    "alias for DEFAULT_DISPATCHER_ID must be wiped on replace_default_inline"
  );
}
