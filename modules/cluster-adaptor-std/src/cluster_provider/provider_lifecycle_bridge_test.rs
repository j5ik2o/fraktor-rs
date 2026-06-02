use alloc::boxed::Box;
use std::{
  string::{String, ToString},
  time::Duration,
  vec::Vec,
};

use fraktor_actor_core_kernel_rs::event::stream::{
  EventStreamEvent, EventStreamShared, EventStreamSubscriber, EventStreamSubscriberShared, EventStreamSubscription,
};
use fraktor_cluster_core_kernel_rs::{
  cluster_provider::{DiscoveryTopologyMapper, LocalClusterProvider, LocalClusterProviderWeak, SeedNodeInput},
  extension::ClusterProviderError,
  topology::{BlockListProvider, ClusterEvent},
};
use fraktor_utils_core_rs::{
  sync::{ArcShared, SharedAccess, SharedLock, SpinSyncMutex},
  time::TimerInstant,
};

use crate::cluster_provider::{
  DiscoveryBackend, DiscoveryBackendError, GenericDiscoveryAdapter, ProviderLifecycleBridge,
  wrap_local_cluster_provider,
};

struct EmptyBlockList;

impl BlockListProvider for EmptyBlockList {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

#[derive(Clone)]
struct RecordingClusterEvents {
  events: SharedLock<Vec<ClusterEvent>>,
}

impl RecordingClusterEvents {
  fn new() -> Self {
    Self { events: SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new()) }
  }

  fn events(&self) -> Vec<ClusterEvent> {
    self.events.with_read(|events| events.clone())
  }
}

impl EventStreamSubscriber for RecordingClusterEvents {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == "cluster"
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
    {
      self.events.with_write(|events| events.push(cluster_event.clone()));
    }
  }
}

#[derive(Clone)]
struct CountingDiscoveryBackend {
  source_identity: String,
  authorities:     SharedLock<Vec<String>>,
  calls:           SharedLock<usize>,
}

impl CountingDiscoveryBackend {
  fn new(source_identity: &str, authorities: Vec<String>) -> Self {
    Self {
      source_identity: source_identity.to_string(),
      authorities:     SharedLock::new_with_driver::<SpinSyncMutex<_>>(authorities),
      calls:           SharedLock::new_with_driver::<SpinSyncMutex<_>>(0),
    }
  }

  fn call_count(&self) -> usize {
    self.calls.with_read(|calls| *calls)
  }

  fn replace_authorities(&self, authorities: Vec<String>) {
    self.authorities.with_write(|current| *current = authorities);
  }
}

impl DiscoveryBackend for CountingDiscoveryBackend {
  fn source_identity(&self) -> &str {
    self.source_identity.as_str()
  }

  fn discover(&mut self) -> Result<Vec<String>, DiscoveryBackendError> {
    self.calls.with_write(|calls| *calls += 1);
    Ok(self.authorities.with_read(|authorities| authorities.clone()))
  }
}

struct FailingDiscoveryBackend {
  source_identity: String,
}

impl FailingDiscoveryBackend {
  fn new(source_identity: &str) -> Self {
    Self { source_identity: source_identity.to_string() }
  }
}

impl DiscoveryBackend for FailingDiscoveryBackend {
  fn source_identity(&self) -> &str {
    self.source_identity.as_str()
  }

  fn discover(&mut self) -> Result<Vec<String>, DiscoveryBackendError> {
    Err(DiscoveryBackendError::temporary("backend unavailable"))
  }
}

fn block_list() -> ArcShared<dyn BlockListProvider> {
  ArcShared::new(EmptyBlockList)
}

fn subscribe_recorder(event_stream: &EventStreamShared) -> (RecordingClusterEvents, EventStreamSubscription) {
  let subscriber_impl = RecordingClusterEvents::new();
  let subscriber = EventStreamSubscriberShared::from_shared_lock(SharedLock::new_with_driver::<SpinSyncMutex<_>>(
    Box::new(subscriber_impl.clone()),
  ));
  let subscription = event_stream.subscribe(&subscriber);
  (subscriber_impl, subscription)
}

fn seed_input(advertised_authority: &str, seed_authorities: Vec<&str>) -> SeedNodeInput {
  SeedNodeInput::new(String::from(advertised_authority), seed_authorities.into_iter().map(String::from).collect())
}

fn mapper() -> DiscoveryTopologyMapper {
  DiscoveryTopologyMapper::new(block_list())
}

fn observed_at() -> TimerInstant {
  TimerInstant::from_ticks(1, Duration::from_secs(1))
}

#[test]
fn provider_lifecycle_bridge_seed_input_publishes_topology_update() {
  let event_stream = EventStreamShared::default();
  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);
  let provider = wrap_local_cluster_provider(LocalClusterProvider::new(event_stream, block_list(), "node-a"));
  let backend = CountingDiscoveryBackend::new("test-discovery", Vec::new());
  let mut bridge = ProviderLifecycleBridge::new(
    provider.downgrade(),
    seed_input("node-a", Vec::from(["node-a", "node-b"])),
    GenericDiscoveryAdapter::new(backend),
    mapper(),
  );

  bridge.start_member().expect("member lifecycle should start");

  let events = subscriber_impl.events();
  let topology_events: Vec<&ClusterEvent> =
    events.iter().filter(|event| matches!(event, ClusterEvent::TopologyUpdated { .. })).collect();
  assert_eq!(topology_events.len(), 1);
  assert!(matches!(
    topology_events[0],
    ClusterEvent::TopologyUpdated { update }
    if update.joined == vec![String::from("node-b")]
      && update.members == vec![String::from("node-a"), String::from("node-b")]
  ));
}

#[test]
fn provider_lifecycle_bridge_member_start_joins_seed_and_discovery_authorities() {
  let event_stream = EventStreamShared::default();
  let provider = wrap_local_cluster_provider(LocalClusterProvider::new(event_stream, block_list(), "node-a"));
  let backend = CountingDiscoveryBackend::new("test-discovery", Vec::from([String::from("node-c")]));
  let mut bridge = ProviderLifecycleBridge::new(
    provider.downgrade(),
    seed_input("node-a", Vec::from(["node-a", "node-b"])),
    GenericDiscoveryAdapter::new(backend),
    mapper(),
  );

  bridge.start_member().expect("member lifecycle should start");

  assert_eq!(provider.with_read(|provider| provider.member_count()), 3);
}

#[test]
fn provider_lifecycle_bridge_validates_seed_before_provider_start() {
  let event_stream = EventStreamShared::default();
  let provider = wrap_local_cluster_provider(LocalClusterProvider::new(event_stream, block_list(), "node-a"));
  let backend = CountingDiscoveryBackend::new("test-discovery", Vec::new());
  let mut bridge = ProviderLifecycleBridge::new(
    provider.downgrade(),
    seed_input("node-a", Vec::from(["invalid seed"])),
    GenericDiscoveryAdapter::new(backend),
    mapper(),
  );

  let result = bridge.start_member();

  assert!(matches!(
    result,
    Err(ClusterProviderError::JoinFailed(ref reason)) if reason == "invalid seed authority"
  ));
  assert_eq!(provider.with_read(|provider| provider.member_count()), 0);
  assert!(!provider.with_read(|provider| provider.is_started()));
}

#[test]
fn provider_lifecycle_bridge_refreshes_discovery_after_member_start() {
  let event_stream = EventStreamShared::default();
  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);
  let provider = wrap_local_cluster_provider(LocalClusterProvider::new(event_stream, block_list(), "node-a"));
  let backend = CountingDiscoveryBackend::new("test-discovery", Vec::from([String::from("node-b")]));
  let backend_probe = backend.clone();
  let mut bridge = ProviderLifecycleBridge::new(
    provider.downgrade(),
    seed_input("node-a", Vec::new()),
    GenericDiscoveryAdapter::new(backend),
    mapper(),
  );

  bridge.start_member().expect("member lifecycle should start");
  backend_probe.replace_authorities(Vec::from([String::from("node-c")]));
  bridge.refresh_discovery().expect("discovery refresh should apply delta");

  let events = subscriber_impl.events();
  let topology_events: Vec<&ClusterEvent> =
    events.iter().filter(|event| matches!(event, ClusterEvent::TopologyUpdated { .. })).collect();
  assert_eq!(backend_probe.call_count(), 2);
  assert_eq!(provider.with_read(|provider| provider.member_count()), 2);
  assert_eq!(topology_events.len(), 3);
  assert!(matches!(
    topology_events[1],
    ClusterEvent::TopologyUpdated { update }
    if update.joined == vec![String::from("node-c")]
      && update.left.is_empty()
      && update.members == vec![String::from("node-a"), String::from("node-b"), String::from("node-c")]
  ));
  assert!(matches!(
    topology_events[2],
    ClusterEvent::TopologyUpdated { update }
    if update.joined.is_empty()
      && update.left == vec![String::from("node-b")]
      && update.members == vec![String::from("node-a"), String::from("node-c")]
  ));
}

#[test]
fn provider_lifecycle_bridge_propagates_backend_failure_without_destroying_topology() {
  let event_stream = EventStreamShared::default();
  let provider = wrap_local_cluster_provider(LocalClusterProvider::new(event_stream, block_list(), "node-a"));
  let mut bridge = ProviderLifecycleBridge::new(
    provider.downgrade(),
    seed_input("node-a", Vec::new()),
    GenericDiscoveryAdapter::new(FailingDiscoveryBackend::new("test-discovery")),
    mapper(),
  );

  let result = bridge.start_member();

  assert!(matches!(
    result,
    Err(ClusterProviderError::StartMemberFailed(ref reason)) if reason == "backend unavailable"
  ));
  assert_eq!(provider.with_read(|provider| provider.member_count()), 1);
}

#[test]
fn provider_lifecycle_bridge_client_start_does_not_register_full_member_self() {
  let event_stream = EventStreamShared::default();
  let provider = wrap_local_cluster_provider(LocalClusterProvider::new(event_stream, block_list(), "node-a"));
  let backend = CountingDiscoveryBackend::new("test-discovery", Vec::from([String::from("node-b")]));
  let mut bridge = ProviderLifecycleBridge::new(
    provider.downgrade(),
    seed_input("node-a", Vec::from(["node-a", "node-b"])),
    GenericDiscoveryAdapter::new(backend),
    mapper(),
  );

  bridge.start_client().expect("client lifecycle should start");

  assert_eq!(provider.with_read(|provider| provider.member_count()), 0);
}

#[test]
fn provider_lifecycle_bridge_shutdown_stops_seed_and_discovery_lifecycle() {
  let event_stream = EventStreamShared::default();
  let provider = wrap_local_cluster_provider(LocalClusterProvider::new(event_stream, block_list(), "node-a"));
  let backend = CountingDiscoveryBackend::new("test-discovery", Vec::from([String::from("node-c")]));
  let backend_probe = backend.clone();
  let mut bridge = ProviderLifecycleBridge::new(
    provider.downgrade(),
    seed_input("node-a", Vec::from(["node-b"])),
    GenericDiscoveryAdapter::new(backend),
    mapper(),
  );

  bridge.start_member().expect("member lifecycle should start");
  bridge.shutdown(true).expect("bridge should shutdown");
  bridge.start_member().expect("stopped bridge should not emit additional join input");

  assert_eq!(backend_probe.call_count(), 1);
  assert_eq!(provider.with_read(|provider| provider.member_count()), 0);
  assert!(bridge.is_shutdown());
}

#[test]
fn provider_lifecycle_bridge_does_not_keep_provider_alive() {
  let weak_provider: LocalClusterProviderWeak;
  let bridge = {
    let event_stream = EventStreamShared::default();
    let provider = wrap_local_cluster_provider(LocalClusterProvider::new(event_stream, block_list(), "node-a"));
    weak_provider = provider.downgrade();
    ProviderLifecycleBridge::new(
      provider.downgrade(),
      seed_input("node-a", Vec::new()),
      GenericDiscoveryAdapter::new(CountingDiscoveryBackend::new("test-discovery", Vec::new())),
      mapper(),
    )
  };

  assert!(weak_provider.upgrade().is_none());
  assert!(!bridge.provider_is_alive());
}

#[test]
fn provider_lifecycle_bridge_observation_time_advances_per_poll() {
  let event_stream = EventStreamShared::default();
  let provider = wrap_local_cluster_provider(LocalClusterProvider::new(event_stream, block_list(), "node-a"));
  let mut bridge = ProviderLifecycleBridge::new(
    provider.downgrade(),
    seed_input("node-a", Vec::new()),
    GenericDiscoveryAdapter::new(CountingDiscoveryBackend::new("test-discovery", Vec::new())),
    mapper(),
  );

  assert_eq!(bridge.next_observed_at(), TimerInstant::from_ticks(1, Duration::from_secs(1)));
  assert_eq!(bridge.next_observed_at(), TimerInstant::from_ticks(2, Duration::from_secs(1)));
}

#[test]
fn generic_discovery_adapter_shutdown_stops_polling() {
  let backend = CountingDiscoveryBackend::new("test-discovery", Vec::from([String::from("node-b")]));
  let backend_probe = backend.clone();
  let mut adapter = GenericDiscoveryAdapter::new(backend);

  assert!(adapter.poll(observed_at()).is_some());
  adapter.shutdown();
  assert!(adapter.poll(observed_at()).is_none());

  assert_eq!(backend_probe.call_count(), 1);
  assert!(adapter.is_shutdown());
}
