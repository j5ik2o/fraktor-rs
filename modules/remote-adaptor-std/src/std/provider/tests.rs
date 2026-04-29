use alloc::{format, vec::Vec};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Address as ActorAddress, Pid,
    actor_path::{ActorPath, ActorPathError, ActorPathParser, ActorPathScheme},
    actor_ref::ActorRef,
    actor_ref_provider::{ActorRefProvider, ActorRefProviderHandleShared, LocalActorRefProvider},
    error::{ActorError, SendError},
    messaging::AnyMessage,
  },
  event::stream::EventStreamEvent,
  routing::{RandomPool, RemoteRouterConfig, RoundRobinPool, Routee, SmallestMailboxPool},
  serialization::ActorRefResolveCache,
};
use fraktor_remote_core_rs::core::{
  address::{Address as RemoteCoreAddress, RemoteNodeId, UniqueAddress},
  extension::{
    REMOTE_ACTOR_REF_RESOLVE_CACHE_EXTENSION, RemoteActorRefResolveCacheEvent, RemoteActorRefResolveCacheOutcome,
  },
  provider::{ProviderError, RemoteActorRef, RemoteActorRefProvider},
};
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};

use crate::std::{
  provider::{
    RemoteRouteeExpansion, RemoteRouteeExpansionError, StdRemoteActorRefProvider, StdRemoteActorRefProviderError,
  },
  tcp_transport::TcpRemoteTransport,
  tests::test_support::EventHarness,
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

fn remote_node_a() -> ActorAddress {
  ActorAddress::remote("remote-a", "10.0.0.1", 2552)
}

fn remote_node_b() -> ActorAddress {
  ActorAddress::remote("remote-b", "10.0.0.2", 2553)
}

fn indexed_worker_path(index: usize, node: &ActorAddress) -> Result<ActorPath, ActorPathError> {
  ActorPathParser::parse(&format!("{}/user/worker-{index}", node.to_uri_string()))
}

fn invalid_worker_path(_index: usize, _node: &ActorAddress) -> Result<ActorPath, ActorPathError> {
  ActorPath::try_from_segments(["user", "worker/invalid"])
}

fn local_worker_path(index: usize, _node: &ActorAddress) -> Result<ActorPath, ActorPathError> {
  Ok(ActorPath::root().child("worker").child(index.to_string()))
}

fn assert_routee_path(routee: &Routee, expected_path: &str) {
  match routee {
    | Routee::ActorRef(actor_ref) => {
      let canonical_path = actor_ref.canonical_path().expect("remote routee should keep canonical path");
      assert_eq!(canonical_path.to_canonical_uri(), expected_path);
    },
    | Routee::NoRoutee | Routee::Several(_) => panic!("remote routee should be an ActorRef"),
  }
}

struct ProviderFixture {
  provider:        StdRemoteActorRefProvider,
  actor_ref_calls: SharedLock<Vec<ActorPath>>,
  event_harness:   EventHarness,
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
  let transport = SharedLock::new_with_driver::<DefaultMutex<_>>(TcpRemoteTransport::new("127.0.0.1:0", Vec::new()));
  let event_harness = EventHarness::new();
  let provider = StdRemoteActorRefProvider::new(
    local_address(),
    local_actor_ref_provider_handle_shared,
    remote_provider,
    transport,
    ActorRefResolveCache::default(),
    event_harness.publisher().clone(),
  );
  ProviderFixture { provider, actor_ref_calls, event_harness }
}

fn make_provider() -> StdRemoteActorRefProvider {
  make_provider_fixture().provider
}

fn make_provider_with_remote_error(error: ProviderError) -> StdRemoteActorRefProvider {
  let local_actor_ref_provider_handle_shared = ActorRefProviderHandleShared::new(LocalActorRefProvider::new());
  let remote_provider = Box::new(RejectingRemoteProvider::new(error)) as Box<dyn RemoteActorRefProvider + Send + Sync>;
  let transport = SharedLock::new_with_driver::<DefaultMutex<_>>(TcpRemoteTransport::new("127.0.0.1:0", Vec::new()));
  let event_harness = EventHarness::new();
  StdRemoteActorRefProvider::new(
    local_address(),
    local_actor_ref_provider_handle_shared,
    remote_provider,
    transport,
    ActorRefResolveCache::default(),
    event_harness.publisher().clone(),
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

#[test]
fn remote_actor_ref_try_tell_fails_until_payload_serialization_is_installed() {
  let mut provider = make_provider();
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  let mut actor_ref = provider.actor_ref(remote_path).expect("remote actor ref should resolve");

  let err = actor_ref.try_tell(AnyMessage::new(String::from("remote-payload"))).unwrap_err();
  let (recovered, context) = match err {
    | SendError::InvalidPayload { message, context } => (message, context),
    | other => panic!("expected payload serialization guard error, got {other:?}"),
  };

  assert_eq!(context, "remote payload serialization is not installed");
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

#[test]
fn remote_routee_expansion_builds_router_with_remote_actor_ref_routees() {
  // Given
  let mut fixture = make_provider_fixture();
  let config = RemoteRouterConfig::new(RoundRobinPool::new(3), vec![remote_node_a(), remote_node_b()]);
  let expansion = RemoteRouteeExpansion::new(config, indexed_worker_path);

  // When
  let router = expansion.expand(&mut fixture.provider).expect("remote routees should expand");

  // Then
  assert_eq!(router.routees().len(), 3);
  assert_routee_path(&router.routees()[0], "fraktor.tcp://remote-a@10.0.0.1:2552/user/worker-0");
  assert_routee_path(&router.routees()[1], "fraktor.tcp://remote-b@10.0.0.2:2553/user/worker-1");
  assert_routee_path(&router.routees()[2], "fraktor.tcp://remote-a@10.0.0.1:2552/user/worker-2");
  assert_eq!(fixture.actor_ref_call_count(), 3);
}

#[test]
fn remote_routee_expansion_supports_smallest_mailbox_pool() {
  // Given
  let mut fixture = make_provider_fixture();
  let config = RemoteRouterConfig::new(SmallestMailboxPool::new(2), vec![remote_node_a(), remote_node_b()]);
  let expansion = RemoteRouteeExpansion::new(config, indexed_worker_path);

  // When
  let router = expansion.expand(&mut fixture.provider).expect("smallest-mailbox routees should expand");

  // Then
  assert_eq!(router.routees().len(), 2);
  assert_routee_path(&router.routees()[0], "fraktor.tcp://remote-a@10.0.0.1:2552/user/worker-0");
  assert_routee_path(&router.routees()[1], "fraktor.tcp://remote-b@10.0.0.2:2553/user/worker-1");
}

#[test]
fn remote_routee_expansion_supports_random_pool() {
  // Given
  let mut fixture = make_provider_fixture();
  let config = RemoteRouterConfig::new(RandomPool::new(2), vec![remote_node_a(), remote_node_b()]);
  let expansion = RemoteRouteeExpansion::new(config, indexed_worker_path);

  // When
  let router = expansion.expand(&mut fixture.provider).expect("random routees should expand");

  // Then
  assert_eq!(router.routees().len(), 2);
  assert_routee_path(&router.routees()[0], "fraktor.tcp://remote-a@10.0.0.1:2552/user/worker-0");
  assert_routee_path(&router.routees()[1], "fraktor.tcp://remote-b@10.0.0.2:2553/user/worker-1");
}

#[test]
fn remote_routee_expansion_reports_path_factory_error_with_routee_index() {
  // Given
  let mut provider = make_provider();
  let config = RemoteRouterConfig::new(RoundRobinPool::new(2), vec![remote_node_a(), remote_node_b()]);
  let expansion = RemoteRouteeExpansion::new(config, invalid_worker_path);

  // When
  let err = match expansion.expand(&mut provider) {
    | Err(err) => err,
    | Ok(_) => panic!("routee path error should fail expansion"),
  };

  // Then
  match err {
    | RemoteRouteeExpansionError::RouteePath { index, .. } => assert_eq!(index, 0),
    | other => panic!("expected routee path error, got {other:?}"),
  }
}

#[test]
fn remote_routee_expansion_reports_provider_error_with_routee_index_and_path() {
  // Given
  let mut fixture = make_provider_fixture();
  let config = RemoteRouterConfig::new(RoundRobinPool::new(2), vec![remote_node_a(), remote_node_b()]);
  let expansion = RemoteRouteeExpansion::new(config, local_worker_path);

  // When
  let err = match expansion.expand(&mut fixture.provider) {
    | Err(err) => err,
    | Ok(_) => panic!("provider error should fail expansion"),
  };

  // Then
  match err {
    | RemoteRouteeExpansionError::Provider { index, path, .. } => {
      assert_eq!(index, 0);
      assert_eq!(path.to_relative_string(), "/user/worker/0");
    },
    | other => panic!("expected provider error, got {other:?}"),
  }
  assert_eq!(fixture.actor_ref_call_count(), 0);
}
