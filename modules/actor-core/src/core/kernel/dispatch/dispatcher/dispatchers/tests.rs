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
  fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
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
fn pekko_compat_id_normalises_to_default() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register(DEFAULT_DISPATCHER_ID, make_default_configurator("default")).expect("register default");
  let resolved = dispatchers.resolve("pekko.actor.default-dispatcher").expect("resolve compat id");
  assert_eq!(resolved.id(), "default");
}

#[test]
fn pekko_internal_dispatcher_id_normalises_to_default() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register(DEFAULT_DISPATCHER_ID, make_default_configurator("default")).expect("register default");
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
