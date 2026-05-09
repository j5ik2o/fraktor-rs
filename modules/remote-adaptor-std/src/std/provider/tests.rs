use alloc::vec::Vec;
use std::time::{Duration, Instant};

use fraktor_actor_adaptor_std_rs::std::{system::std_actor_system_config, tick_driver::TestTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_path::{ActorPath, ActorPathParser, ActorPathScheme},
    actor_ref::ActorRef,
    actor_ref_provider::{ActorRefProvider, ActorRefProviderHandleShared, LocalActorRefProvider},
    error::{ActorError, SendError},
    extension::ExtensionInstallers,
    messaging::AnyMessage,
  },
  event::stream::EventStreamEvent,
  serialization::ActorRefResolveCache,
  system::ActorSystem,
};
use fraktor_remote_core_rs::core::{
  address::{Address as RemoteCoreAddress, RemoteNodeId, UniqueAddress},
  config::RemoteConfig,
  extension::{
    REMOTE_ACTOR_REF_RESOLVE_CACHE_EXTENSION, RemoteActorRefResolveCacheEvent, RemoteActorRefResolveCacheOutcome,
    RemoteEvent,
  },
  provider::{ProviderError, RemoteActorRef, RemoteActorRefProvider},
  transport::TransportEndpoint,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, DefaultMutex, SharedLock};
use tokio::{
  sync::mpsc::{self, Receiver, Sender},
  time::timeout,
};

use super::{StdRemoteActorRefProvider, StdRemoteActorRefProviderError, StdRemoteActorRefProviderInstaller};
use crate::std::{
  extension_installer::RemotingExtensionInstaller, tests::test_support::EventHarness,
  transport::tcp::TcpRemoteTransport,
};

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/// Tracks every call so tests can assert the dispatch path.
struct StubRemoteProvider {
  actor_ref_calls: SharedLock<Vec<ActorPath>>,
  watch_calls:     Vec<(ActorPath, Pid)>,
  unwatch_calls:   Vec<(ActorPath, Pid)>,
}

impl StubRemoteProvider {
  fn new(actor_ref_calls: SharedLock<Vec<ActorPath>>) -> Self {
    Self { actor_ref_calls, watch_calls: Vec::new(), unwatch_calls: Vec::new() }
  }
}

impl RemoteActorRefProvider for StubRemoteProvider {
  fn actor_ref(&mut self, path: ActorPath) -> Result<RemoteActorRef, ProviderError> {
    self.actor_ref_calls.with_lock(|calls| calls.push(path.clone()));
    let node = RemoteNodeId::new("remote", "10.0.0.1", Some(2552), 1);
    Ok(RemoteActorRef::new(path, node))
  }

  fn watch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), ProviderError> {
    self.watch_calls.push((watchee, watcher));
    Ok(())
  }

  fn unwatch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), ProviderError> {
    self.unwatch_calls.push((watchee, watcher));
    Ok(())
  }
}

struct RejectingRemoteProvider {
  error: ProviderError,
}

impl RejectingRemoteProvider {
  const fn new(error: ProviderError) -> Self {
    Self { error }
  }
}

impl RemoteActorRefProvider for RejectingRemoteProvider {
  fn actor_ref(&mut self, _path: ActorPath) -> Result<RemoteActorRef, ProviderError> {
    Err(self.error.clone())
  }

  fn watch(&mut self, _watchee: ActorPath, _watcher: Pid) -> Result<(), ProviderError> {
    Err(self.error.clone())
  }

  fn unwatch(&mut self, _watchee: ActorPath, _watcher: Pid) -> Result<(), ProviderError> {
    Err(self.error.clone())
  }
}

fn local_address() -> UniqueAddress {
  UniqueAddress::new(RemoteCoreAddress::new("local-sys", "127.0.0.1", 2551), 7)
}

struct ProviderFixture {
  provider:        StdRemoteActorRefProvider,
  actor_ref_calls: SharedLock<Vec<ActorPath>>,
  event_harness:   EventHarness,
  event_rx:        Receiver<RemoteEvent>,
}

impl ProviderFixture {
  fn actor_ref_call_count(&self) -> usize {
    self.actor_ref_calls.with_lock(|calls| calls.len())
  }

  fn resolve_cache_events(&self) -> Vec<RemoteActorRefResolveCacheEvent> {
    self
      .event_harness
      .events()
      .into_iter()
      .filter_map(|event| match event {
        | EventStreamEvent::Extension { name, payload } if name == REMOTE_ACTOR_REF_RESOLVE_CACHE_EXTENSION => {
          payload.downcast_ref::<RemoteActorRefResolveCacheEvent>().cloned()
        },
        | _ => None,
      })
      .collect()
  }
}

fn make_provider_fixture() -> ProviderFixture {
  let local_actor_ref_provider_handle_shared = ActorRefProviderHandleShared::new(LocalActorRefProvider::new());
  let actor_ref_calls = SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new());
  let remote_provider =
    Box::new(StubRemoteProvider::new(actor_ref_calls.clone())) as Box<dyn RemoteActorRefProvider + Send + Sync>;
  let (event_tx, event_rx) = mpsc::channel(8);
  let event_harness = EventHarness::new();
  let provider = StdRemoteActorRefProvider::new(
    local_address(),
    local_actor_ref_provider_handle_shared,
    remote_provider,
    event_tx,
    ActorRefResolveCache::default(),
    event_harness.publisher().clone(),
    Instant::now(),
  );
  ProviderFixture { provider, actor_ref_calls, event_harness, event_rx }
}

fn make_provider() -> StdRemoteActorRefProvider {
  make_provider_fixture().provider
}

fn make_provider_with_event_sender(event_sender: Sender<RemoteEvent>) -> StdRemoteActorRefProvider {
  let local_actor_ref_provider_handle_shared = ActorRefProviderHandleShared::new(LocalActorRefProvider::new());
  let actor_ref_calls = SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new());
  let remote_provider =
    Box::new(StubRemoteProvider::new(actor_ref_calls)) as Box<dyn RemoteActorRefProvider + Send + Sync>;
  let event_harness = EventHarness::new();
  StdRemoteActorRefProvider::new(
    local_address(),
    local_actor_ref_provider_handle_shared,
    remote_provider,
    event_sender,
    ActorRefResolveCache::default(),
    event_harness.publisher().clone(),
    Instant::now(),
  )
}

fn make_provider_with_remote_error(error: ProviderError) -> StdRemoteActorRefProvider {
  let local_actor_ref_provider_handle_shared = ActorRefProviderHandleShared::new(LocalActorRefProvider::new());
  let remote_provider = Box::new(RejectingRemoteProvider::new(error)) as Box<dyn RemoteActorRefProvider + Send + Sync>;
  let (event_tx, _event_rx) = mpsc::channel(8);
  let event_harness = EventHarness::new();
  StdRemoteActorRefProvider::new(
    local_address(),
    local_actor_ref_provider_handle_shared,
    remote_provider,
    event_tx,
    ActorRefResolveCache::default(),
    event_harness.publisher().clone(),
    Instant::now(),
  )
}

fn assert_remote_actor_ref_path(result: Result<ActorRef, StdRemoteActorRefProviderError>, expected_path: &ActorPath) {
  let actor_ref = result.expect("remote actor ref should resolve");
  let canonical_path = actor_ref.canonical_path().expect("remote actor ref canonical path");
  assert_eq!(canonical_path.to_canonical_uri(), expected_path.to_canonical_uri());
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn local_path_without_authority_is_dispatched_to_local_provider() {
  let mut provider = make_provider();
  let local_path = ActorPath::root().child("user").child("worker");
  // The unconfigured local provider returns an error, but we only care that
  // the call lands on `LocalProvider(...)` (not on `CoreProvider`).
  let err = provider.actor_ref(local_path).unwrap_err();
  assert!(matches!(err, StdRemoteActorRefProviderError::LocalProvider(_)), "expected LocalProvider error, got {err:?}");
}

#[test]
fn remote_path_with_non_matching_authority_is_dispatched_to_remote_provider() {
  let mut provider = make_provider();
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.99:2552/user/worker").expect("parse");
  let result = provider.actor_ref(remote_path.clone());
  assert_remote_actor_ref_path(result, &remote_path);
}

#[test]
fn local_authority_path_is_normalized_to_local_provider() {
  let mut provider = make_provider();
  // Authority that exactly matches `local_address()`.
  let local_path = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker").expect("parse");
  let err = provider.actor_ref(local_path).unwrap_err();
  assert!(
    matches!(err, StdRemoteActorRefProviderError::LocalProvider(_)),
    "expected LocalProvider error (loopback dispatched to local provider), got {err:?}"
  );
}

#[test]
fn local_authority_path_with_uid_zero_is_treated_as_wildcard() {
  let mut provider = make_provider();
  // `#0` UID is a wildcard per design Decision 13 — Address match alone
  // should still trigger the loopback branch even though the local UID is 7.
  let local_path = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker#0").expect("parse");
  let err = provider.actor_ref(local_path).unwrap_err();
  assert!(
    matches!(err, StdRemoteActorRefProviderError::LocalProvider(_)),
    "expected LocalProvider error (wildcard UID dispatched to local), got {err:?}"
  );
}

#[test]
fn local_authority_path_with_non_matching_uid_is_dispatched_to_remote() {
  let mut provider = make_provider();
  let local_path = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker#99").expect("parse");
  let result = provider.actor_ref(local_path.clone());
  assert_remote_actor_ref_path(result, &local_path);
}

#[test]
fn remote_actor_ref_resolution_uses_cache_after_first_miss() {
  let mut fixture = make_provider_fixture();
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");

  let first = fixture.provider.actor_ref(remote_path.clone());
  let second = fixture.provider.actor_ref(remote_path.clone());

  assert_remote_actor_ref_path(first, &remote_path);
  assert_remote_actor_ref_path(second, &remote_path);
  assert_eq!(fixture.actor_ref_call_count(), 1);
}

#[test]
fn remote_actor_ref_resolution_reuses_cached_actor_ref_pid() {
  let mut fixture = make_provider_fixture();
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");

  let first = fixture.provider.actor_ref(remote_path.clone()).expect("first remote actor ref should resolve");
  let second = fixture.provider.actor_ref(remote_path).expect("second remote actor ref should resolve");

  assert_eq!(first.pid(), second.pid());
  assert_eq!(fixture.actor_ref_call_count(), 1);
}

#[test]
fn remote_actor_ref_resolution_publishes_cache_miss_then_hit_events() {
  let mut fixture = make_provider_fixture();
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");

  let first = fixture.provider.actor_ref(remote_path.clone());
  let second = fixture.provider.actor_ref(remote_path.clone());

  assert_remote_actor_ref_path(first, &remote_path);
  assert_remote_actor_ref_path(second, &remote_path);
  let events = fixture.resolve_cache_events();
  assert_eq!(events.len(), 2);
  assert_eq!(events[0].path(), &remote_path);
  assert_eq!(events[0].outcome(), RemoteActorRefResolveCacheOutcome::Miss);
  assert_eq!(events[1].path(), &remote_path);
  assert_eq!(events[1].outcome(), RemoteActorRefResolveCacheOutcome::Hit);
}

fn assert_outbound_enqueued_event(
  event: RemoteEvent,
  expected_authority: &str,
  expected_system: &str,
  expected_path: &ActorPath,
) {
  match event {
    | RemoteEvent::OutboundEnqueued { authority, envelope, .. } => {
      assert_eq!(authority, TransportEndpoint::new(expected_authority));
      assert_eq!(envelope.recipient(), expected_path);
      assert_eq!(envelope.sender(), None);
      assert_eq!(envelope.remote_node().system(), expected_system);
      assert_eq!(envelope.remote_node().host(), "10.0.0.1");
      assert_eq!(envelope.remote_node().port(), Some(2552));
    },
    | other => panic!("expected OutboundEnqueued, got {other:?}"),
  }
}

#[test]
fn remote_actor_ref_try_tell_pushes_outbound_enqueued_event() {
  let mut fixture = make_provider_fixture();
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  let mut actor_ref = fixture.provider.actor_ref(remote_path.clone()).expect("remote actor ref should resolve");

  actor_ref.try_tell(AnyMessage::new(String::from("remote-payload"))).expect("remote send should enqueue event");

  let event = fixture.event_rx.try_recv().expect("outbound event should be available");
  assert_outbound_enqueued_event(event, "remote@10.0.0.1:2552", "remote", &remote_path);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn actor_system_config_registered_std_remote_actor_ref_provider_resolves_remote_actor_ref() {
  let remote_installer = ArcShared::new(RemotingExtensionInstaller::new(
    TcpRemoteTransport::new("127.0.0.1:0", vec![local_address().address().clone()]),
    RemoteConfig::new("127.0.0.1"),
  ));
  let extension_installers = ExtensionInstallers::default().with_shared_extension_installer(remote_installer.clone());
  let installer =
    StdRemoteActorRefProviderInstaller::from_remoting_extension_installer(local_address(), remote_installer);
  let config = std_actor_system_config(TestTickDriver::default())
    .with_extension_installers(extension_installers)
    .with_actor_ref_provider_installer(installer);
  let system = ActorSystem::create_with_noop_guardian(config).expect("system");
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  let mut actor_ref = system.resolve_actor_ref(remote_path.clone()).expect("remote actor ref should resolve");
  let canonical_path = actor_ref.canonical_path().expect("remote actor ref canonical path");
  assert_eq!(canonical_path.to_canonical_uri(), remote_path.to_canonical_uri());

  actor_ref.try_tell(AnyMessage::new(String::from("remote-payload"))).expect("remote send should enqueue event");

  system.terminate().expect("terminate");
  timeout(Duration::from_secs(1), system.when_terminated()).await.expect("system should terminate");
}

#[test]
fn remote_actor_ref_try_tell_returns_full_when_event_channel_is_full() {
  let (event_tx, _event_rx) = mpsc::channel(1);
  let mut provider = make_provider_with_event_sender(event_tx);
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  let mut actor_ref = provider.actor_ref(remote_path).expect("remote actor ref should resolve");

  actor_ref.try_tell(AnyMessage::new(String::from("first"))).expect("first send should fill event channel");
  let err = actor_ref.try_tell(AnyMessage::new(String::from("second"))).unwrap_err();

  let recovered = match err {
    | SendError::Full(message) => message,
    | other => panic!("expected full send error, got {other:?}"),
  };
  assert_eq!(recovered.downcast_ref::<String>().map(String::as_str), Some("second"));
}

#[test]
fn remote_actor_ref_try_tell_returns_closed_when_event_channel_is_closed() {
  let (event_tx, event_rx) = mpsc::channel(1);
  drop(event_rx);
  let mut provider = make_provider_with_event_sender(event_tx);
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  let mut actor_ref = provider.actor_ref(remote_path).expect("remote actor ref should resolve");

  let err = actor_ref.try_tell(AnyMessage::new(String::from("remote-payload"))).unwrap_err();

  let recovered = match err {
    | SendError::Closed(message) => message,
    | other => panic!("expected closed send error, got {other:?}"),
  };
  assert_eq!(recovered.downcast_ref::<String>().map(String::as_str), Some("remote-payload"));
}

#[test]
fn actor_ref_provider_handle_shared_resolves_remote_path_through_std_provider_trait() {
  let provider = ActorRefProviderHandleShared::new(make_provider());
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");

  let actor_ref = provider.get_actor_ref(remote_path.clone()).expect("trait provider should resolve remote path");

  assert_eq!(provider.supported_schemes(), &[ActorPathScheme::FraktorTcp]);
  let canonical_path = actor_ref.canonical_path().expect("remote actor ref canonical path");
  assert_eq!(canonical_path.to_canonical_uri(), remote_path.to_canonical_uri());
}

#[test]
fn actor_ref_provider_trait_preserves_local_provider_actor_error_classification() {
  let mut provider = make_provider();
  let local_path = ActorPath::root().child("user").child("worker");

  let err = ActorRefProvider::actor_ref(&mut provider, local_path).unwrap_err();

  assert!(matches!(err, ActorError::Fatal(_)));
  assert_eq!(err.reason().as_str(), "LocalActorRefProvider is not bound to a system state");
}

#[test]
fn actor_ref_provider_trait_maps_core_input_errors_to_escalate() {
  let mut provider = make_provider_with_remote_error(ProviderError::UnsupportedScheme);
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");

  let err = ActorRefProvider::actor_ref(&mut provider, remote_path).unwrap_err();

  assert!(matches!(err, ActorError::Escalate(_)));
  assert_eq!(err.reason().as_str(), "std remote provider: core provider error: provider: unsupported path scheme");
}

#[test]
fn watch_remote_path_forwards_to_remote_provider() {
  let mut provider = make_provider();
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  provider.watch(remote_path, Pid::new(1, 1)).expect("watch should succeed");
}

#[test]
fn unwatch_remote_path_forwards_to_remote_provider() {
  let mut provider = make_provider();
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  provider.unwatch(remote_path, Pid::new(1, 1)).expect("unwatch should succeed");
}

#[test]
fn watch_local_path_returns_not_remote() {
  let mut provider = make_provider();
  let local_path = ActorPath::root().child("user").child("worker");
  let err = provider.watch(local_path, Pid::new(1, 1)).unwrap_err();
  assert!(matches!(err, StdRemoteActorRefProviderError::NotRemote));
}

#[test]
fn unwatch_local_path_returns_not_remote() {
  let mut provider = make_provider();
  let local_path = ActorPath::root().child("user").child("worker");
  let err = provider.unwatch(local_path, Pid::new(1, 1)).unwrap_err();
  assert!(matches!(err, StdRemoteActorRefProviderError::NotRemote));
}

#[test]
fn watch_local_authority_path_returns_not_remote() {
  let mut provider = make_provider();
  // Authority matches local — should be treated as local for watch purposes.
  let local_path = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker").expect("parse");
  let err = provider.watch(local_path, Pid::new(1, 1)).unwrap_err();
  assert!(matches!(err, StdRemoteActorRefProviderError::NotRemote));
}
