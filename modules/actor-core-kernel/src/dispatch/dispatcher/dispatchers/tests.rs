use alloc::boxed::Box;
use core::time::Duration;

use fraktor_utils_core_rs::sync::ArcShared;

use super::{DEFAULT_BLOCKING_DISPATCHER_ID, DEFAULT_DISPATCHER_ID, Dispatchers, DispatchersError};
use crate::dispatch::dispatcher::{
  DefaultDispatcherFactory, DispatcherConfig, ExecuteError, Executor, ExecutorShared, MessageDispatcherFactory,
  TrampolineState,
};

struct NoopExecutor;

impl Executor for NoopExecutor {
  fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    Ok(())
  }

  fn shutdown(&mut self) {}
}

fn make_default_configurator(id: &str) -> ArcShared<Box<dyn MessageDispatcherFactory>> {
  let settings = DispatcherConfig::with_defaults(id).with_shutdown_timeout(Duration::from_secs(2));
  let executor = ExecutorShared::new(Box::new(NoopExecutor), TrampolineState::new());
  let configurator: Box<dyn MessageDispatcherFactory> = Box::new(DefaultDispatcherFactory::new(&settings, executor));
  ArcShared::new(configurator)
}

#[test]
fn register_then_resolve_returns_same_dispatcher() {
  let mut dispatchers = Dispatchers::new();
  let configurator = make_default_configurator(DEFAULT_DISPATCHER_ID);
  dispatchers.register(DEFAULT_DISPATCHER_ID, configurator).expect("register");
  let shared = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve");
  assert_eq!(shared.id(), DEFAULT_DISPATCHER_ID);
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
fn pekko_default_dispatcher_id_resolves_as_primary_entry_after_ensure_default() {
  // DEFAULT_DISPATCHER_ID == "pekko.actor.default-dispatcher" (flip 後の primary entry id)。
  // alias ではなく entry 直接 lookup になることを確認する。
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default(|| make_default_configurator(DEFAULT_DISPATCHER_ID));
  let resolved = dispatchers.resolve("fraktor.actor.default-dispatcher").expect("resolve primary entry");
  assert_eq!(resolved.id(), DEFAULT_DISPATCHER_ID);
  // canonical_id も同じ id を返す (alias chain を辿らず即時 entry 一致)。
  assert_eq!(dispatchers.canonical_id("fraktor.actor.default-dispatcher").expect("canonical"), DEFAULT_DISPATCHER_ID);
}

#[test]
fn pekko_internal_dispatcher_id_resolves_via_alias_registered_by_ensure_default() {
  // internal-dispatcher は引き続き alias として primary entry に解決される
  // (Pekko `InternalDispatcherId` 互換のため)。
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default(|| make_default_configurator(DEFAULT_DISPATCHER_ID));
  let resolved = dispatchers.resolve("fraktor.actor.internal-dispatcher").expect("resolve internal");
  assert_eq!(resolved.id(), DEFAULT_DISPATCHER_ID);
}

#[test]
fn legacy_default_id_is_retired_and_returns_unknown() {
  // 完全退役の回帰防止: fraktor-rs 独自の legacy 短縮表記 `"default"` は本 change で
  // entry でも alias でも登録されなくなったため、resolve / canonical_id は Unknown を返す。
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default_inline();
  match dispatchers.resolve("default") {
    | Ok(_) => panic!("legacy `\"default\"` must be retired and must not resolve"),
    | Err(DispatchersError::Unknown(id)) => assert_eq!(id, "default"),
    | Err(other) => panic!("expected Unknown, got {other:?}"),
  }
  match dispatchers.canonical_id("default") {
    | Ok(id) => panic!("legacy canonical_id resolved unexpectedly: {id}"),
    | Err(DispatchersError::Unknown(id)) => assert_eq!(id, "default"),
    | Err(other) => panic!("expected Unknown, got {other:?}"),
  }
}

#[test]
fn ensure_default_inline_registers_only_internal_dispatcher_alias() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default_inline();
  // aliases map には internal-dispatcher の 1 件のみ存在 (legacy "default" は登録されない)。
  assert_eq!(dispatchers.aliases.len(), 1);
  assert_eq!(
    dispatchers.aliases.get("fraktor.actor.internal-dispatcher").map(String::as_str),
    Some(DEFAULT_DISPATCHER_ID)
  );
  assert!(!dispatchers.aliases.contains_key("default"), "legacy default must not be an alias");
  assert!(!dispatchers.aliases.contains_key(DEFAULT_DISPATCHER_ID), "primary entry must not be its own alias");
}

#[test]
fn ensure_default_inserts_when_missing() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default(|| make_default_configurator(DEFAULT_DISPATCHER_ID));
  let resolved = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve default");
  assert_eq!(resolved.id(), DEFAULT_DISPATCHER_ID);
  let blocking = dispatchers.resolve(DEFAULT_BLOCKING_DISPATCHER_ID).expect("resolve blocking");
  assert_eq!(blocking.id(), DEFAULT_DISPATCHER_ID);
}

#[test]
fn ensure_default_is_idempotent_when_present() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register(DEFAULT_DISPATCHER_ID, make_default_configurator("first")).expect("register");
  dispatchers.ensure_default(|| make_default_configurator("second"));
  // 既存 configurator はそのまま維持される (ensure_default は idempotent)。
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
  dispatchers.register(DEFAULT_DISPATCHER_ID, make_default_configurator(DEFAULT_DISPATCHER_ID)).expect("register");
  assert_eq!(dispatchers.resolve_call_count(), 0);
  let _ = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve 1");
  assert_eq!(dispatchers.resolve_call_count(), 1);
  let _ = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve 2");
  let _ = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve 3");
  assert_eq!(dispatchers.resolve_call_count(), 3);
}

#[test]
fn resolve_call_count_increments_even_on_unknown_id() {
  let dispatchers = Dispatchers::new();
  assert_eq!(dispatchers.resolve_call_count(), 0);
  let _ = dispatchers.resolve("missing");
  let _ = dispatchers.resolve("missing");
  // 失敗 lookup も counter をインクリメントする (成功分だけでなく registry への
  // 全 call traffic を diagnostic に捕捉するため)。
  assert_eq!(dispatchers.resolve_call_count(), 2);
}

#[test]
fn resolve_call_count_is_shared_across_clones() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register(DEFAULT_DISPATCHER_ID, make_default_configurator(DEFAULT_DISPATCHER_ID)).expect("register");
  let cloned = dispatchers.clone();
  let _ = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve from original");
  let _ = cloned.resolve(DEFAULT_DISPATCHER_ID).expect("resolve from clone");
  // clone 同士は同じ counter を共有するため、どの Dispatchers handle が観測した
  // 呼び出しであっても diagnostic には合算される。
  assert_eq!(dispatchers.resolve_call_count(), 2);
  assert_eq!(cloned.resolve_call_count(), 2);
}

// --- Alias chain tests (change `pekko-dispatcher-alias-chain`, spec Scenarios 1-9) ---

#[test]
fn single_hop_alias_resolves_to_target_entry() {
  let mut dispatchers = Dispatchers::new();
  dispatchers
    .register(DEFAULT_DISPATCHER_ID, make_default_configurator(DEFAULT_DISPATCHER_ID))
    .expect("register default");
  dispatchers.register_alias("app.work", DEFAULT_DISPATCHER_ID).expect("register alias");

  let before = dispatchers.resolve_call_count();
  let resolved = dispatchers.resolve("app.work").expect("resolve alias");
  assert_eq!(resolved.id(), DEFAULT_DISPATCHER_ID);
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
fn alias_chain_at_max_depth_resolves_to_terminal_entry() {
  // off-by-one regression 防止: MAX_ALIAS_DEPTH 段ちょうどの alias chain は成功で解決される
  // べきであり、depth-exceeded で拒否されてはならない。本テストは「0〜MAX_ALIAS_DEPTH 段を
  // 辿る」契約 (`spec` Requirement) を境界値で固定する。
  let mut dispatchers = Dispatchers::new();
  let terminal = alloc::format!("alias_{}", Dispatchers::MAX_ALIAS_DEPTH);
  dispatchers.register(&terminal, make_default_configurator(&terminal)).expect("register terminal entry");
  for step in 0..Dispatchers::MAX_ALIAS_DEPTH {
    let alias = alloc::format!("alias_{step}");
    let target = alloc::format!("alias_{}", step + 1);
    dispatchers.register_alias(alias, target).expect("alias chain step");
  }

  let resolved = dispatchers.resolve("alias_0").expect("resolve at MAX_ALIAS_DEPTH must succeed");
  assert_eq!(resolved.id(), terminal);
}

#[test]
fn alias_chain_exceeding_max_depth_returns_alias_chain_too_deep() {
  let mut dispatchers = Dispatchers::new();
  // (MAX_ALIAS_DEPTH + 1) 段の線形 alias chain を構築する。末尾 id は entry として
  // 登録しないため、先頭を resolve すると Unknown ではなく depth-exceeded が返ること
  // を確認する。
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
  dispatchers
    .register(DEFAULT_DISPATCHER_ID, make_default_configurator(DEFAULT_DISPATCHER_ID))
    .expect("register default");
  dispatchers.register_alias("foo", DEFAULT_DISPATCHER_ID).expect("register alias");

  match dispatchers.register("foo", make_default_configurator("foo")) {
    | Err(DispatchersError::AliasConflictsWithEntry(id)) => assert_eq!(id, "foo"),
    | other => panic!("expected AliasConflictsWithEntry, got {other:?}"),
  }

  // 既存 alias エントリは保持され、引き続き primary entry に解決される。
  let resolved = dispatchers.resolve("foo").expect("resolve");
  assert_eq!(resolved.id(), DEFAULT_DISPATCHER_ID);
}

#[test]
fn register_alias_rejects_id_already_registered_as_entry() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register("foo", make_default_configurator("foo")).expect("register foo");

  match dispatchers.register_alias("foo", DEFAULT_DISPATCHER_ID) {
    | Err(DispatchersError::AliasConflictsWithEntry(id)) => assert_eq!(id, "foo"),
    | other => panic!("expected AliasConflictsWithEntry, got {other:?}"),
  }

  // 既存 entry は保持される。
  let resolved = dispatchers.resolve("foo").expect("resolve foo");
  assert_eq!(resolved.id(), "foo");
}

#[test]
fn register_alias_rejects_duplicate_alias() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register_alias("foo", DEFAULT_DISPATCHER_ID).expect("first alias");

  match dispatchers.register_alias("foo", "other") {
    | Err(DispatchersError::Duplicate(id)) => assert_eq!(id, "foo"),
    | other => panic!("expected Duplicate, got {other:?}"),
  }

  // 既存 alias target は保持される。
  assert_eq!(dispatchers.aliases.get("foo").map(String::as_str), Some(DEFAULT_DISPATCHER_ID));
}

#[test]
fn register_or_update_is_lenient_and_wipes_existing_alias() {
  // flip 後は Pekko `default-dispatcher` は primary entry なので、ユーザー定義の任意 alias を
  // 使って wipe 挙動を検証する。
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default(|| make_default_configurator(DEFAULT_DISPATCHER_ID));
  dispatchers.register_alias("app.my-alias", DEFAULT_DISPATCHER_ID).expect("register alias");
  let via_alias = dispatchers.resolve("app.my-alias").expect("resolve via user alias");
  assert_eq!(via_alias.id(), DEFAULT_DISPATCHER_ID);

  // register_or_update (builder 経路) で alias を上書きして具体 entry にする。
  // 戻り値は unit (infallible) であり、この呼び出しで alias は wipe される。
  let custom = make_default_configurator("app-my-alias-custom");
  dispatchers.register_or_update("app.my-alias", custom);

  let resolved = dispatchers.resolve("app.my-alias").expect("resolve after override");
  assert_eq!(resolved.id(), "app-my-alias-custom");

  // 以前 alias が指していた target 側 (primary entry) は変更されない。
  let still_primary = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve primary");
  assert_eq!(still_primary.id(), DEFAULT_DISPATCHER_ID);
}

#[test]
fn canonical_id_returns_resolved_entry_id() {
  // flip 後は internal-dispatcher のみが alias なので、これを canonical_id で解決する。
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default(|| make_default_configurator(DEFAULT_DISPATCHER_ID));

  let canonical = dispatchers.canonical_id("fraktor.actor.internal-dispatcher").expect("canonical");
  assert_eq!(canonical, DEFAULT_DISPATCHER_ID);

  // canonical_id は resolve counter をインクリメントしない。
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
  // Bugbot の指摘シナリオを再現: 呼び出し側が先に `DEFAULT_DISPATCHER_ID` 宛ての alias を登録し、
  // その後 `ensure_default` を呼ぶケース。alias を wipe しないと、`follow_alias_chain` が `aliases`
  // を先に参照するため、新規 insert した entry が alias 経由で shadow されてしまう。
  let mut dispatchers = Dispatchers::new();
  dispatchers.register_alias(DEFAULT_DISPATCHER_ID, "some-other-id").expect("pre-existing alias");

  dispatchers.ensure_default(|| make_default_configurator(DEFAULT_DISPATCHER_ID));

  let resolved = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve default after wipe");
  assert_eq!(resolved.id(), DEFAULT_DISPATCHER_ID);
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
  assert_eq!(resolved.id(), DEFAULT_DISPATCHER_ID);
  assert!(
    !dispatchers.aliases.contains_key(DEFAULT_DISPATCHER_ID),
    "alias for DEFAULT_DISPATCHER_ID must be wiped on replace_default_inline"
  );
}
