use alloc::vec::Vec;
use std::{
  thread,
  time::{Duration, Instant},
};

use fraktor_actor_adaptor_std_rs::{
  system::{create_noop_actor_system_with, std_actor_system_config},
  tick_driver::TestTickDriver,
};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Address as ActorAddress, Pid,
    actor_path::{ActorPath, ActorPathParser, ActorPathScheme},
    actor_ref::ActorRef,
    actor_ref_provider::{
      ActorRefProvider, ActorRefProviderHandleShared, LocalActorRefProvider, LocalActorRefProviderInstaller,
    },
    deploy::RemoteScope,
    error::{ActorError, SendError},
    extension::ExtensionInstallers,
    messaging::{AnyMessage, system_message::SystemMessage},
    props::DeployablePropsMetadata,
  },
  event::stream::{CorrelationId, EventStreamEvent},
  serialization::default_serialization_extension_id,
  system::{
    ActorSystem,
    remote::{RemoteDeploymentHook, RemoteDeploymentOutcome, RemoteDeploymentRequest, RemoteWatchHook},
  },
};
use fraktor_remote_core_rs::{
  address::{Address as RemoteCoreAddress, RemoteNodeId, UniqueAddress},
  association::QuarantineReason,
  config::RemoteConfig,
  envelope::{OutboundEnvelope, OutboundPriority},
  extension::{
    EventPublisher, REMOTE_ACTOR_REF_RESOLVE_CACHE_EXTENSION, Remote, RemoteActorRefResolveCacheEvent,
    RemoteActorRefResolveCacheOutcome, RemoteEvent, RemoteShared,
  },
  provider::{ProviderError, RemoteActorRef, RemoteActorRefProvider},
  transport::{RemoteTransport, TransportEndpoint, TransportError},
  watcher::WatcherCommand,
  wire::{
    AckPdu, ControlPdu, HandshakePdu, HandshakeRsp, RemoteDeploymentCreateFailure, RemoteDeploymentCreateSuccess,
    RemoteDeploymentFailureCode, RemoteDeploymentPdu,
  },
};
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock};
use tokio::{
  sync::mpsc::{self, Receiver, Sender},
  time::timeout,
};

use super::{
  StdRemoteActorRefProvider, StdRemoteActorRefProviderError, StdRemoteActorRefProviderInstaller,
  remote_actor_path_registry::RemoteActorPathRegistry,
  remote_deployment_hook::StdRemoteDeploymentHook,
  remote_watch_hook::{StdRemoteWatchFlushConfig, StdRemoteWatchHook},
};
use crate::{
  deployment::{DeploymentResponse, DeploymentResponseDispatcher},
  extension_installer::{RemotingExtensionInstaller, StdFlushGate},
  tests::test_support_test::EventHarness,
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

struct NoopRemoteTransport {
  addresses: Vec<RemoteCoreAddress>,
}

impl NoopRemoteTransport {
  fn new(addresses: Vec<RemoteCoreAddress>) -> Self {
    Self { addresses }
  }
}

impl RemoteTransport for NoopRemoteTransport {
  fn start(&mut self) -> Result<(), TransportError> {
    Ok(())
  }

  fn shutdown(&mut self) -> Result<(), TransportError> {
    Ok(())
  }

  fn connect_peer(&mut self, _remote: &RemoteCoreAddress) -> Result<(), TransportError> {
    Ok(())
  }

  fn send(&mut self, _envelope: OutboundEnvelope) -> Result<(), (TransportError, Box<OutboundEnvelope>)> {
    Ok(())
  }

  fn send_control(&mut self, _remote: &RemoteCoreAddress, _pdu: ControlPdu) -> Result<(), TransportError> {
    Ok(())
  }

  fn send_flush_request(
    &mut self,
    _remote: &RemoteCoreAddress,
    _pdu: ControlPdu,
    _lane_id: u32,
  ) -> Result<(), TransportError> {
    Ok(())
  }

  fn send_ack(&mut self, _remote: &RemoteCoreAddress, _pdu: AckPdu) -> Result<(), TransportError> {
    Ok(())
  }

  fn send_handshake(&mut self, _remote: &RemoteCoreAddress, _pdu: HandshakePdu) -> Result<(), TransportError> {
    Ok(())
  }

  fn schedule_handshake_timeout(
    &mut self,
    _authority: &TransportEndpoint,
    _timeout: Duration,
    _generation: u64,
  ) -> Result<(), TransportError> {
    Ok(())
  }

  fn addresses(&self) -> &[RemoteCoreAddress] {
    &self.addresses
  }

  fn default_address(&self) -> Option<&RemoteCoreAddress> {
    self.addresses.first()
  }

  fn local_address_for_remote(&self, _remote: &RemoteCoreAddress) -> Option<&RemoteCoreAddress> {
    self.default_address()
  }

  fn quarantine(
    &mut self,
    _address: &RemoteCoreAddress,
    _uid: Option<u64>,
    _reason: QuarantineReason,
  ) -> Result<(), TransportError> {
    Ok(())
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
  registry:        SharedLock<RemoteActorPathRegistry>,
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
  let registry = RemoteActorPathRegistry::new_shared();
  let provider = StdRemoteActorRefProvider::new_with_registry(
    local_address(),
    local_actor_ref_provider_handle_shared,
    remote_provider,
    event_tx,
    event_harness.publisher().clone(),
    registry.clone(),
    Instant::now(),
  );
  ProviderFixture { provider, actor_ref_calls, event_harness, event_rx, registry }
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
  StdRemoteActorRefProvider::new_with_registry(
    local_address(),
    local_actor_ref_provider_handle_shared,
    remote_provider,
    event_sender,
    event_harness.publisher().clone(),
    RemoteActorPathRegistry::new_shared(),
    Instant::now(),
  )
}

fn make_provider_with_remote_error(error: ProviderError) -> StdRemoteActorRefProvider {
  let local_actor_ref_provider_handle_shared = ActorRefProviderHandleShared::new(LocalActorRefProvider::new());
  let remote_provider = Box::new(RejectingRemoteProvider::new(error)) as Box<dyn RemoteActorRefProvider + Send + Sync>;
  let (event_tx, _event_rx) = mpsc::channel(8);
  let event_harness = EventHarness::new();
  StdRemoteActorRefProvider::new_with_registry(
    local_address(),
    local_actor_ref_provider_handle_shared,
    remote_provider,
    event_tx,
    event_harness.publisher().clone(),
    RemoteActorPathRegistry::new_shared(),
    Instant::now(),
  )
}

fn assert_remote_actor_ref_path(result: Result<ActorRef, StdRemoteActorRefProviderError>, expected_path: &ActorPath) {
  let actor_ref = result.expect("remote actor ref should resolve");
  let canonical_path = actor_ref.canonical_path().expect("remote actor ref canonical path");
  assert_eq!(canonical_path.to_canonical_uri(), expected_path.to_canonical_uri());
}

fn remote_actor_path() -> ActorPath {
  ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse")
}

fn noop_transport_envelope(target: &RemoteCoreAddress) -> OutboundEnvelope {
  OutboundEnvelope::new(
    remote_actor_path(),
    None,
    AnyMessage::new(String::from("payload")),
    OutboundPriority::User,
    RemoteNodeId::new(target.system(), target.host(), Some(target.port()), 1),
    CorrelationId::nil(),
  )
}

fn register_local_path(system: &ActorSystem, pid: Pid, name: &str) -> ActorPath {
  let path = ActorPath::root().child("user").child(name);
  system.state().register_actor_path(pid, &path);
  system.state().canonical_actor_path(&pid).expect("registered path should have canonical form")
}

fn make_remote_watch_hook_fixture(
  registry: SharedLock<RemoteActorPathRegistry>,
) -> (StdRemoteWatchHook, Receiver<RemoteEvent>, Receiver<WatcherCommand>, EventHarness) {
  make_remote_watch_hook_fixture_with_capacities(registry, 8, 8)
}

fn make_remote_watch_hook_fixture_with_watcher_capacity(
  registry: SharedLock<RemoteActorPathRegistry>,
  watcher_capacity: usize,
) -> (StdRemoteWatchHook, Receiver<RemoteEvent>, Receiver<WatcherCommand>, EventHarness) {
  make_remote_watch_hook_fixture_with_capacities(registry, 8, watcher_capacity)
}

fn make_remote_watch_hook_fixture_with_capacities(
  registry: SharedLock<RemoteActorPathRegistry>,
  event_capacity: usize,
  watcher_capacity: usize,
) -> (StdRemoteWatchHook, Receiver<RemoteEvent>, Receiver<WatcherCommand>, EventHarness) {
  let harness = EventHarness::new();
  let (event_tx, event_rx) = mpsc::channel(event_capacity);
  let (watcher_tx, watcher_rx) = mpsc::channel(watcher_capacity);
  let flush_config = remote_watch_flush_config(&harness);
  let hook = StdRemoteWatchHook::new_with_flush_gate(
    registry,
    harness.system().state(),
    event_tx,
    watcher_tx,
    Instant::now(),
    flush_config,
  );
  (hook, event_rx, watcher_rx, harness)
}

fn remote_watch_flush_config(harness: &EventHarness) -> StdRemoteWatchFlushConfig {
  let serialization_extension = harness.system().extended().register_extension(&default_serialization_extension_id());
  let remote = Remote::new(
    NoopRemoteTransport::new(vec![RemoteCoreAddress::new("local-sys", "127.0.0.1", 0)]),
    RemoteConfig::new("127.0.0.1"),
    EventPublisher::new(harness.system().downgrade()),
    serialization_extension,
  );
  StdRemoteWatchFlushConfig::new(RemoteShared::new(remote), StdFlushGate::default(), vec![0])
}

fn deployment_request(
  child_name: &str,
  target: ActorAddress,
  metadata: Option<DeployablePropsMetadata>,
) -> RemoteDeploymentRequest {
  RemoteDeploymentRequest::new(
    Pid::new(41, 0),
    Pid::new(42, 0),
    String::from(child_name),
    ActorPath::root().child(child_name),
    RemoteScope::new(target),
    metadata,
  )
}

fn deployable_metadata() -> DeployablePropsMetadata {
  DeployablePropsMetadata::new("echo", AnyMessage::new(String::from("payload")))
}

fn remote_deployment_hook_fixture(
  timeout: Duration,
) -> (StdRemoteDeploymentHook, mpsc::Receiver<RemoteEvent>, DeploymentResponseDispatcher, ActorSystem) {
  let system = create_noop_actor_system_with(|config| {
    config.with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default())
  });
  let serialization = system.extended().register_extension(&default_serialization_extension_id());
  let (event_sender, event_rx) = mpsc::channel(8);
  let dispatcher = DeploymentResponseDispatcher::default();
  let hook = StdRemoteDeploymentHook::new(
    local_address(),
    system.clone(),
    event_sender,
    Instant::now(),
    serialization,
    dispatcher.clone(),
    timeout,
  );
  (hook, event_rx, dispatcher, system)
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
fn remote_deployment_hook_loopback_returns_local_outcome() {
  let (mut hook, mut event_rx, _dispatcher, _harness) = remote_deployment_hook_fixture(Duration::from_millis(50));
  let request = deployment_request(
    "loopback-deploy",
    ActorAddress::remote("local-sys", "127.0.0.1", 2551),
    Some(deployable_metadata()),
  );

  let outcome = hook.deploy_child(request);

  assert!(matches!(outcome, RemoteDeploymentOutcome::UseLocalDeployment));
  assert!(event_rx.try_recv().is_err());
}

#[test]
fn remote_deployment_hook_rejects_non_deployable_props_without_enqueue() {
  let (mut hook, mut event_rx, _dispatcher, _harness) = remote_deployment_hook_fixture(Duration::from_millis(50));
  let request = deployment_request("non-deployable", ActorAddress::remote("remote-sys", "10.0.0.1", 2552), None);

  let outcome = hook.deploy_child(request);

  assert!(matches!(outcome, RemoteDeploymentOutcome::Failed(reason) if reason.contains("deployable props metadata")));
  assert!(event_rx.try_recv().is_err());
}

#[test]
fn remote_deployment_hook_enqueues_create_and_resolves_matching_success() {
  let (mut hook, mut event_rx, dispatcher, system) = remote_deployment_hook_fixture(Duration::from_secs(1));
  let request = deployment_request(
    "remote-deploy",
    ActorAddress::remote("remote-sys", "10.0.0.1", 2552),
    Some(deployable_metadata()),
  );
  let success_path = system.user_guardian_ref().canonical_path().expect("user guardian path").to_canonical_uri();
  let join = thread::spawn(move || hook.deploy_child(request));

  let event = event_rx.blocking_recv().expect("deployment request should be enqueued");
  let (correlation_hi, correlation_lo) = match event {
    | RemoteEvent::OutboundDeployment { remote, pdu: RemoteDeploymentPdu::CreateRequest(request), .. } => {
      assert_eq!(remote, RemoteCoreAddress::new("remote-sys", "10.0.0.1", 2552));
      assert_eq!(request.target_parent_path(), "fraktor.tcp://remote-sys@10.0.0.1:2552/user");
      assert_eq!(request.child_name(), "remote-deploy");
      assert_eq!(request.factory_id(), "echo");
      assert_eq!(request.origin_node(), "local-sys@127.0.0.1:2551");
      (request.correlation_hi(), request.correlation_lo())
    },
    | other => panic!("expected deployment create request, got {other:?}"),
  };
  dispatcher.complete(DeploymentResponse::Success(RemoteDeploymentCreateSuccess::new(
    correlation_hi,
    correlation_lo,
    success_path.clone(),
  )));

  let outcome = join.join().expect("deployment hook thread should complete");
  let RemoteDeploymentOutcome::RemoteCreated(actor_ref) = outcome else {
    panic!("expected remote actor ref, got {outcome:?}");
  };
  let canonical = actor_ref.canonical_path().expect("resolved ref should keep canonical path");
  assert_eq!(canonical.to_canonical_uri(), success_path);
}

#[test]
fn remote_deployment_hook_timeout_is_bounded_and_cancels_pending() {
  let (mut hook, mut event_rx, dispatcher, _harness) = remote_deployment_hook_fixture(Duration::from_millis(10));
  let request = deployment_request(
    "timeout-deploy",
    ActorAddress::remote("remote-sys", "10.0.0.1", 2552),
    Some(deployable_metadata()),
  );

  let outcome = hook.deploy_child(request);
  let event = event_rx.try_recv().expect("deployment request should be enqueued before timeout");
  let (correlation_hi, correlation_lo) = match event {
    | RemoteEvent::OutboundDeployment { pdu: RemoteDeploymentPdu::CreateRequest(request), .. } => {
      (request.correlation_hi(), request.correlation_lo())
    },
    | other => panic!("expected deployment create request, got {other:?}"),
  };
  assert!(matches!(outcome, RemoteDeploymentOutcome::Failed(reason) if reason.contains("timed out")));

  dispatcher.complete(DeploymentResponse::Failure(RemoteDeploymentCreateFailure::new(
    correlation_hi,
    correlation_lo,
    RemoteDeploymentFailureCode::SpawnFailed,
    String::from("late"),
  )));
  assert_eq!(dispatcher.stale_len(), 1);
}

#[test]
fn deployment_response_dispatcher_records_unknown_response_as_stale() {
  let dispatcher = DeploymentResponseDispatcher::default();

  dispatcher.complete(DeploymentResponse::Failure(RemoteDeploymentCreateFailure::new(
    7,
    0,
    RemoteDeploymentFailureCode::SpawnFailed,
    String::from("unknown"),
  )));

  assert_eq!(dispatcher.stale_len(), 1);
}

#[test]
fn noop_remote_transport_fixture_handles_provider_flush_contract_without_io() {
  let local = RemoteCoreAddress::new("local-sys", "127.0.0.1", 2551);
  let remote = RemoteCoreAddress::new("remote-sys", "10.0.0.1", 2552);
  let mut transport = NoopRemoteTransport::new(vec![local.clone()]);

  assert_eq!(transport.addresses(), [local.clone()].as_slice());
  assert_eq!(transport.default_address(), Some(&local));
  assert_eq!(transport.local_address_for_remote(&remote), Some(&local));

  transport.start().expect("start should be a no-op");
  transport.connect_peer(&remote).expect("connect should be a no-op");
  transport.send(noop_transport_envelope(&remote)).expect("send should be a no-op");
  let control = ControlPdu::Shutdown { authority: local.to_string() };
  transport.send_control(&remote, control.clone()).expect("control send should be a no-op");
  transport.send_flush_request(&remote, control, 0).expect("flush request should be a no-op");
  transport.send_ack(&remote, AckPdu::new(1, 1, 0)).expect("ack should be a no-op");
  transport
    .send_handshake(&remote, HandshakePdu::Rsp(HandshakeRsp::new(UniqueAddress::new(local.clone(), 1))))
    .expect("handshake should be a no-op");
  transport
    .schedule_handshake_timeout(&TransportEndpoint::new(remote.to_string()), Duration::from_millis(1), 1)
    .expect("handshake timeout should be a no-op");
  transport.quarantine(&remote, Some(1), QuarantineReason::new("test")).expect("quarantine should be a no-op");
  transport.shutdown().expect("shutdown should be a no-op");
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
fn remote_actor_ref_resolution_records_pid_path_mapping() {
  let mut fixture = make_provider_fixture();
  let remote_path = remote_actor_path();

  let actor_ref = fixture.provider.actor_ref(remote_path.clone()).expect("remote actor ref should resolve");

  let recorded = fixture.registry.with_lock(|registry| registry.path_for_pid(&actor_ref.pid()));
  assert_eq!(recorded.as_ref().map(ActorPath::to_canonical_uri), Some(remote_path.to_canonical_uri()));
}

#[test]
fn remote_actor_ref_sender_removes_pid_path_mapping_after_last_ref_is_dropped() {
  let mut fixture = make_provider_fixture();
  let registry = fixture.registry.clone();
  let remote_path = remote_actor_path();

  let actor_ref = fixture.provider.actor_ref(remote_path).expect("remote actor ref should resolve");
  let pid = actor_ref.pid();
  drop(fixture.provider);

  assert!(registry.with_lock(|registry| registry.path_for_pid(&pid)).is_some());
  drop(actor_ref);
  assert!(registry.with_lock(|registry| registry.path_for_pid(&pid)).is_none());
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

#[test]
fn remote_watch_hook_forwards_watch_command() {
  let registry = RemoteActorPathRegistry::new_shared();
  let remote_pid = Pid::new(900, 0);
  let remote_path = remote_actor_path();
  registry.with_lock(|registry| registry.record(remote_pid, remote_path.clone()));
  let (mut hook, _event_rx, mut watcher_rx, harness) = make_remote_watch_hook_fixture(registry);
  let local_pid = Pid::new(901, 0);
  let local_path = register_local_path(harness.system(), local_pid, "watcher");

  assert!(hook.handle_watch(remote_pid, local_pid));

  let command = watcher_rx.try_recv().expect("watch command should be enqueued");
  assert!(matches!(
    command,
    WatcherCommand::Watch { target, watcher }
      if target.to_canonical_uri() == remote_path.to_canonical_uri()
        && watcher.to_canonical_uri() == local_path.to_canonical_uri()
  ));
}

#[test]
fn remote_watch_hook_treats_full_watcher_queue_as_handled() {
  let registry = RemoteActorPathRegistry::new_shared();
  let remote_pid = Pid::new(905, 0);
  let remote_path = remote_actor_path();
  registry.with_lock(|registry| registry.record(remote_pid, remote_path));
  let (mut hook, _event_rx, mut watcher_rx, harness) =
    make_remote_watch_hook_fixture_with_watcher_capacity(registry, 1);
  let local_pid = Pid::new(906, 0);
  let _local_path = register_local_path(harness.system(), local_pid, "watcher-full");

  assert!(hook.handle_watch(remote_pid, local_pid));
  assert!(hook.handle_watch(remote_pid, local_pid));
  assert!(matches!(watcher_rx.try_recv(), Ok(WatcherCommand::Watch { .. })));
  assert!(watcher_rx.try_recv().is_err());
}

#[test]
fn remote_watch_hook_returns_true_when_watcher_queue_is_closed() {
  let registry = RemoteActorPathRegistry::new_shared();
  let remote_pid = Pid::new(907, 0);
  let remote_path = remote_actor_path();
  registry.with_lock(|registry| registry.record(remote_pid, remote_path));
  let (mut hook, _event_rx, watcher_rx, harness) = make_remote_watch_hook_fixture(registry);
  let local_pid = Pid::new(908, 0);
  let _local_path = register_local_path(harness.system(), local_pid, "watcher-closed");
  drop(watcher_rx);

  assert!(hook.handle_watch(remote_pid, local_pid));
}

#[test]
fn remote_watch_hook_returns_false_when_watcher_path_is_unresolved() {
  let registry = RemoteActorPathRegistry::new_shared();
  let remote_pid = Pid::new(909, 0);
  let remote_path = remote_actor_path();
  registry.with_lock(|registry| registry.record(remote_pid, remote_path));
  let (mut hook, _event_rx, mut watcher_rx, _harness) = make_remote_watch_hook_fixture(registry);

  assert!(!hook.handle_watch(remote_pid, Pid::new(909, 1)));
  assert!(watcher_rx.try_recv().is_err());
}

#[test]
fn remote_watch_hook_forwards_unwatch_command() {
  let registry = RemoteActorPathRegistry::new_shared();
  let remote_pid = Pid::new(910, 0);
  let remote_path = remote_actor_path();
  registry.with_lock(|registry| registry.record(remote_pid, remote_path.clone()));
  let (mut hook, _event_rx, mut watcher_rx, harness) = make_remote_watch_hook_fixture(registry);
  let local_pid = Pid::new(911, 0);
  let local_path = register_local_path(harness.system(), local_pid, "watcher");

  assert!(hook.handle_unwatch(remote_pid, local_pid));

  let command = watcher_rx.try_recv().expect("unwatch command should be enqueued");
  assert!(matches!(
    command,
    WatcherCommand::Unwatch { target, watcher }
      if target.to_canonical_uri() == remote_path.to_canonical_uri()
        && watcher.to_canonical_uri() == local_path.to_canonical_uri()
  ));
}

#[test]
fn remote_watch_hook_returns_false_when_mapping_is_unresolved() {
  let registry = RemoteActorPathRegistry::new_shared();
  let (mut hook, _event_rx, mut watcher_rx, _harness) = make_remote_watch_hook_fixture(registry);

  assert!(!hook.handle_watch(Pid::new(920, 0), Pid::new(921, 0)));
  assert!(watcher_rx.try_recv().is_err());
}

#[test]
fn remote_watch_hook_returns_false_when_unwatch_mapping_is_unresolved() {
  let registry = RemoteActorPathRegistry::new_shared();
  let (mut hook, _event_rx, mut watcher_rx, _harness) = make_remote_watch_hook_fixture(registry);

  assert!(!hook.handle_unwatch(Pid::new(920, 0), Pid::new(921, 0)));
  assert!(watcher_rx.try_recv().is_err());
}

#[test]
fn remote_watch_hook_returns_false_when_unwatch_watcher_path_is_unresolved() {
  let registry = RemoteActorPathRegistry::new_shared();
  let remote_pid = Pid::new(922, 0);
  let remote_path = remote_actor_path();
  registry.with_lock(|registry| registry.record(remote_pid, remote_path));
  let (mut hook, _event_rx, mut watcher_rx, _harness) = make_remote_watch_hook_fixture(registry);

  assert!(!hook.handle_unwatch(remote_pid, Pid::new(922, 1)));
  assert!(watcher_rx.try_recv().is_err());
}

#[test]
fn remote_watch_hook_returns_false_when_deathwatch_watcher_mapping_is_unresolved() {
  let registry = RemoteActorPathRegistry::new_shared();
  let (mut hook, mut event_rx, _watcher_rx, _harness) = make_remote_watch_hook_fixture(registry);

  assert!(!hook.handle_deathwatch_notification(Pid::new(923, 0), Pid::new(923, 1)));
  assert!(event_rx.try_recv().is_err());
}

#[test]
fn remote_watch_hook_returns_false_when_notification_recipient_is_not_remote() {
  let registry = RemoteActorPathRegistry::new_shared();
  let watcher_pid = Pid::new(924, 0);
  registry.with_lock(|registry| registry.record(watcher_pid, ActorPath::root().child("user").child("local")));
  let (mut hook, mut event_rx, _watcher_rx, harness) = make_remote_watch_hook_fixture(registry);
  let terminated_pid = Pid::new(924, 1);
  let _terminated_path = register_local_path(harness.system(), terminated_pid, "terminated-local");

  assert!(!hook.handle_deathwatch_notification(watcher_pid, terminated_pid));
  assert!(event_rx.try_recv().is_err());
}

#[test]
fn remote_watch_hook_forwards_deathwatch_notification_envelope() {
  let registry = RemoteActorPathRegistry::new_shared();
  let remote_watcher_pid = Pid::new(930, 0);
  let remote_watcher_path = remote_actor_path();
  registry.with_lock(|registry| registry.record(remote_watcher_pid, remote_watcher_path.clone()));
  let (mut hook, mut event_rx, _watcher_rx, harness) = make_remote_watch_hook_fixture(registry);
  let terminated_pid = Pid::new(931, 0);
  let terminated_path = register_local_path(harness.system(), terminated_pid, "terminated");

  assert!(hook.handle_deathwatch_notification(remote_watcher_pid, terminated_pid));

  let event = event_rx.try_recv().expect("notification envelope should be enqueued");
  assert!(matches!(
    event,
    RemoteEvent::OutboundEnqueued { authority, envelope, .. }
      if authority == TransportEndpoint::new("remote-sys@10.0.0.1:2552")
        && envelope.priority() == OutboundPriority::System
        && envelope.recipient().to_canonical_uri() == remote_watcher_path.to_canonical_uri()
        && envelope.sender().map(ActorPath::to_canonical_uri) == Some(terminated_path.to_canonical_uri())
        && envelope.message().downcast_ref::<SystemMessage>()
          == Some(&SystemMessage::DeathWatchNotification(terminated_pid))
  ));
}

#[test]
fn remote_watch_hook_forwards_deathwatch_notification_without_sender_when_terminated_path_is_unknown() {
  let registry = RemoteActorPathRegistry::new_shared();
  let remote_watcher_pid = Pid::new(932, 0);
  let remote_watcher_path = remote_actor_path();
  registry.with_lock(|registry| registry.record(remote_watcher_pid, remote_watcher_path.clone()));
  let (mut hook, mut event_rx, _watcher_rx, _harness) = make_remote_watch_hook_fixture(registry);
  let terminated_pid = Pid::new(933, 0);

  assert!(hook.handle_deathwatch_notification(remote_watcher_pid, terminated_pid));

  let event = event_rx.try_recv().expect("notification envelope should be enqueued");
  assert!(matches!(
    event,
    RemoteEvent::OutboundEnqueued { authority, envelope, .. }
      if authority == TransportEndpoint::new("remote-sys@10.0.0.1:2552")
        && envelope.priority() == OutboundPriority::System
        && envelope.recipient().to_canonical_uri() == remote_watcher_path.to_canonical_uri()
        && envelope.sender().is_none()
        && envelope.message().downcast_ref::<SystemMessage>()
          == Some(&SystemMessage::DeathWatchNotification(terminated_pid))
  ));
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_watch_hook_defers_notification_when_event_queue_is_full() {
  let registry = RemoteActorPathRegistry::new_shared();
  let remote_watcher_pid = Pid::new(934, 0);
  let remote_watcher_path = remote_actor_path();
  registry.with_lock(|registry| registry.record(remote_watcher_pid, remote_watcher_path));
  let (mut hook, mut event_rx, _watcher_rx, harness) = make_remote_watch_hook_fixture_with_capacities(registry, 1, 8);
  let terminated_pid = Pid::new(935, 0);
  let _terminated_path = register_local_path(harness.system(), terminated_pid, "terminated-full");

  assert!(hook.handle_deathwatch_notification(remote_watcher_pid, terminated_pid));
  assert!(hook.handle_deathwatch_notification(remote_watcher_pid, terminated_pid));
  assert!(matches!(event_rx.try_recv(), Ok(RemoteEvent::OutboundEnqueued { .. })));
  let deferred = timeout(Duration::from_millis(50), event_rx.recv())
    .await
    .expect("deferred notification should be delivered")
    .expect("event queue should remain open");
  assert!(matches!(deferred, RemoteEvent::OutboundEnqueued { .. }));
  assert!(event_rx.try_recv().is_err());
}

#[test]
fn remote_watch_hook_returns_true_when_notification_event_queue_is_closed() {
  let registry = RemoteActorPathRegistry::new_shared();
  let remote_watcher_pid = Pid::new(936, 0);
  let remote_watcher_path = remote_actor_path();
  registry.with_lock(|registry| registry.record(remote_watcher_pid, remote_watcher_path));
  let (mut hook, event_rx, _watcher_rx, harness) = make_remote_watch_hook_fixture(registry);
  let terminated_pid = Pid::new(937, 0);
  let _terminated_path = register_local_path(harness.system(), terminated_pid, "terminated-closed");
  drop(event_rx);

  assert!(hook.handle_deathwatch_notification(remote_watcher_pid, terminated_pid));
}
