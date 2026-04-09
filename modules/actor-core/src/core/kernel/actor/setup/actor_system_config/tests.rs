use alloc::boxed::Box;
use core::{
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::kernel::{
  actor::{
    actor_path::GuardianKind as PathGuardianKind,
    actor_ref::{ActorRefSender, ActorRefSenderShared},
    setup::ActorSystemConfig,
  },
  dispatch::dispatcher::{DEFAULT_DISPATCHER_ID, Executor, ExecutorShared, MessageDispatcher, MessageDispatcherShared},
  system::{
    lock_provider::{ActorLockProvider, BuiltinSpinLockProvider, MailboxSharedSet},
    remote::RemotingConfig,
  },
};

struct CountingLockProvider {
  dispatcher_shared_calls: ArcShared<AtomicUsize>,
  executor_shared_calls:   ArcShared<AtomicUsize>,
}

impl CountingLockProvider {
  fn new() -> (ArcShared<AtomicUsize>, ArcShared<AtomicUsize>, Self) {
    let dispatcher_shared_calls = ArcShared::new(AtomicUsize::new(0));
    let executor_shared_calls = ArcShared::new(AtomicUsize::new(0));
    let provider = Self {
      dispatcher_shared_calls: dispatcher_shared_calls.clone(),
      executor_shared_calls:   executor_shared_calls.clone(),
    };
    (executor_shared_calls, dispatcher_shared_calls, provider)
  }
}

impl ActorLockProvider for CountingLockProvider {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    self.dispatcher_shared_calls.fetch_add(1, Ordering::SeqCst);
    BuiltinSpinLockProvider::new().create_message_dispatcher_shared(dispatcher)
  }

  fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared {
    self.executor_shared_calls.fetch_add(1, Ordering::SeqCst);
    BuiltinSpinLockProvider::new().create_executor_shared(executor)
  }

  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    BuiltinSpinLockProvider::new().create_actor_ref_sender_shared(sender)
  }

  fn create_mailbox_shared_set(&self) -> MailboxSharedSet {
    BuiltinSpinLockProvider::new().create_mailbox_shared_set()
  }
}

#[test]
fn test_actor_system_config_default() {
  let config = ActorSystemConfig::default();
  assert_eq!(config.system_name(), "default-system");
  assert_eq!(config.default_guardian(), PathGuardianKind::User);
  assert!(config.remoting_config().is_none());
}

#[test]
fn test_actor_system_config_with_system_name() {
  let config = ActorSystemConfig::default().with_system_name("test-system");
  assert_eq!(config.system_name(), "test-system");
}

#[test]
fn test_actor_system_config_with_default_guardian() {
  let config = ActorSystemConfig::default().with_default_guardian(PathGuardianKind::System);
  assert_eq!(config.default_guardian(), PathGuardianKind::System);
}

#[test]
fn test_actor_system_config_with_remoting() {
  let remoting = RemotingConfig::default().with_canonical_host("localhost").with_canonical_port(2552);

  let config = ActorSystemConfig::default().with_remoting_config(remoting);

  assert!(config.remoting_config().is_some());
  let remoting_cfg = config.remoting_config().unwrap();
  assert_eq!(remoting_cfg.canonical_host(), "localhost");
  assert_eq!(remoting_cfg.canonical_port(), Some(2552));
}

#[test]
fn test_remoting_config_quarantine_duration() {
  let custom_duration = Duration::from_secs(1800);
  let remoting = RemotingConfig::default().with_quarantine_duration(custom_duration);

  assert_eq!(remoting.quarantine_duration(), custom_duration);
}

#[test]
fn test_remoting_config_defaults() {
  let remoting = RemotingConfig::default();

  // デフォルト値の検証
  assert_eq!(remoting.canonical_host(), "localhost");
  assert_eq!(remoting.canonical_port(), None);
  assert_eq!(remoting.quarantine_duration(), Duration::from_secs(5 * 24 * 3600)); // 5日
}

#[test]
#[should_panic(expected = "quarantine duration must be >= 1 second")]
fn test_remoting_config_rejects_short_quarantine() {
  drop(RemotingConfig::default().with_quarantine_duration(Duration::from_millis(999)));
}

#[test]
fn test_actor_system_config_default_resolves_default_dispatcher() {
  let config = ActorSystemConfig::default();
  assert!(
    config.dispatchers().resolve(DEFAULT_DISPATCHER_ID).is_ok(),
    "ActorSystemConfig::default() should seed the default dispatcher entry"
  );
}

#[test]
fn test_actor_system_config_with_lock_provider_rebuilds_default_dispatcher() {
  let (executor_shared_calls, dispatcher_shared_calls, provider) = CountingLockProvider::new();

  let config = ActorSystemConfig::default().with_lock_provider(provider);

  assert_eq!(
    executor_shared_calls.load(Ordering::SeqCst),
    1,
    "Replacing the lock provider should rebuild the default dispatcher executor wrapper"
  );
  assert_eq!(
    dispatcher_shared_calls.load(Ordering::SeqCst),
    1,
    "Replacing the lock provider should rebuild the default dispatcher shared wrapper"
  );
  assert!(
    config.dispatchers().resolve(DEFAULT_DISPATCHER_ID).is_ok(),
    "Rebuilt default dispatcher should remain resolvable"
  );
}
