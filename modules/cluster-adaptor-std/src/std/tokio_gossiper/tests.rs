use core::time::Duration;

use fraktor_actor_core_kernel_rs::event::stream::EventStreamShared;
use fraktor_cluster_core_rs::core::{
  ClusterExtensionConfig,
  failure_detector::{DefaultFailureDetectorRegistry, FailureDetector},
  membership::{
    Gossiper, MembershipCoordinator, MembershipCoordinatorConfig, MembershipCoordinatorShared, MembershipTable,
  },
};
use fraktor_remote_core_rs::core::{address::Address, failure_detector::PhiAccrualFailureDetector};
use tokio::runtime::Handle;

use crate::std::{TokioGossipTransport, TokioGossipTransportConfig, TokioGossiper, TokioGossiperConfig};

/// Test-only adapter that bridges the remote-core detector to the
/// cluster-core `FailureDetector` trait.
struct PhiAccrualAdapter(PhiAccrualFailureDetector);

impl FailureDetector for PhiAccrualAdapter {
  fn is_available(&self, now_ms: u64) -> bool {
    self.0.is_available(now_ms)
  }

  fn is_monitoring(&self) -> bool {
    self.0.is_monitoring()
  }

  fn heartbeat(&mut self, now_ms: u64) {
    self.0.heartbeat(now_ms);
  }
}

fn build_coordinator() -> MembershipCoordinatorShared {
  let config = MembershipCoordinatorConfig {
    phi_threshold:          1.0,
    suspect_timeout:        Duration::from_secs(1),
    dead_timeout:           Duration::from_secs(1),
    quarantine_ttl:         Duration::from_secs(1),
    gossip_enabled:         true,
    gossip_interval:        Duration::from_millis(50),
    topology_emit_interval: Duration::from_millis(50),
  };
  let table = MembershipTable::new(3);
  let threshold = config.phi_threshold;
  let registry = DefaultFailureDetectorRegistry::new(Box::new(move || {
    Box::new(PhiAccrualAdapter(PhiAccrualFailureDetector::new(detector_address(), threshold, 10, 1, 0, 10)))
  }));
  let mut coordinator = MembershipCoordinator::new(config, ClusterExtensionConfig::new(), table, registry);
  coordinator.start_member().expect("start_member");
  MembershipCoordinatorShared::new(coordinator)
}

fn detector_address() -> Address {
  Address::new("cluster-test", "127.0.0.1", 0)
}

#[tokio::test]
async fn start_then_stop_is_ok() {
  let config = TokioGossiperConfig::new(Duration::from_millis(50), Duration::from_millis(50));
  let coordinator = build_coordinator();
  let event_stream = EventStreamShared::default();
  let transport = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8),
    Handle::current(),
  )
  .expect("transport bind");

  let mut gossiper = TokioGossiper::new(config, coordinator, transport, event_stream, Handle::current());
  assert!(gossiper.start().is_ok());
  assert!(gossiper.stop().is_ok());
}

#[tokio::test]
async fn stop_without_start_returns_err() {
  let config = TokioGossiperConfig::new(Duration::from_millis(50), Duration::from_millis(50));
  let coordinator = build_coordinator();
  let event_stream = EventStreamShared::default();
  let transport = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8),
    Handle::current(),
  )
  .expect("transport bind");

  let mut gossiper = TokioGossiper::new(config, coordinator, transport, event_stream, Handle::current());
  assert!(gossiper.stop().is_err());
}

#[tokio::test]
async fn start_twice_returns_err() {
  let config = TokioGossiperConfig::new(Duration::from_millis(50), Duration::from_millis(50));
  let coordinator = build_coordinator();
  let event_stream = EventStreamShared::default();
  let transport = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8),
    Handle::current(),
  )
  .expect("transport bind");

  let mut gossiper = TokioGossiper::new(config, coordinator, transport, event_stream, Handle::current());
  assert!(gossiper.start().is_ok());
  assert!(gossiper.start().is_err());
  let _ = gossiper.stop();
}
