use std::{
  string::{String, ToString},
  time::Duration,
  vec::Vec,
};

use fraktor_actor_core_kernel_rs::event::stream::EventStreamShared;
use fraktor_cluster_core_kernel_rs::{
  cluster_provider::{DiscoveryTopologyMapper, LocalClusterProvider, LocalClusterProviderWeak, SeedNodeInput},
  topology::BlockListProvider,
};
use fraktor_utils_core_rs::{
  sync::{ArcShared, SharedAccess, SpinSyncMutex},
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
struct CountingDiscoveryBackend {
  source_identity: String,
  authorities:     Vec<String>,
  calls:           ArcShared<SpinSyncMutex<usize>>,
}

impl CountingDiscoveryBackend {
  fn new(source_identity: &str, authorities: Vec<String>) -> Self {
    Self { source_identity: source_identity.to_string(), authorities, calls: ArcShared::new(SpinSyncMutex::new(0)) }
  }

  fn call_count(&self) -> usize {
    *self.calls.lock()
  }
}

impl DiscoveryBackend for CountingDiscoveryBackend {
  fn source_identity(&self) -> &str {
    self.source_identity.as_str()
  }

  fn discover(&mut self) -> Result<Vec<String>, DiscoveryBackendError> {
    *self.calls.lock() += 1;
    Ok(self.authorities.clone())
  }
}

fn block_list() -> ArcShared<dyn BlockListProvider> {
  ArcShared::new(EmptyBlockList)
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
