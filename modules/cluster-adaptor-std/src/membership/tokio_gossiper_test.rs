use core::time::Duration;

use fraktor_actor_core_kernel_rs::event::stream::EventStreamShared;
use fraktor_cluster_core_kernel_rs::{
  extension::{ClusterExtensionConfig, ClusterProviderError},
  failure_detector::{DefaultFailureDetectorRegistry, FailureDetectorConfig},
  membership::{
    Gossiper, MembershipCoordinator, MembershipCoordinatorConfig, MembershipCoordinatorError,
    MembershipCoordinatorShared, MembershipTable,
  },
};
use fraktor_remote_core_rs::address::Address;
use tokio::runtime::Handle;

use super::should_continue_after_poll_error;
use crate::membership::{
  ConfiguredPhiAccrualDetectorFactory, TokioGossipTransport, TokioGossipTransportConfig, TokioGossiper,
  TokioGossiperConfig,
};

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
  let cluster_config = ClusterExtensionConfig::new()
    .with_failure_detector_config(FailureDetectorConfig::new().with_phi_threshold(config.phi_threshold));
  let detector_config = *cluster_config.failure_detector_config();
  let registry = DefaultFailureDetectorRegistry::new(Box::new(move || {
    ConfiguredPhiAccrualDetectorFactory::new(detector_config, detector_address()).create()
  }));
  let mut coordinator = MembershipCoordinator::new(config, cluster_config, table, registry);
  coordinator.start_member().expect("start_member");
  MembershipCoordinatorShared::new(coordinator)
}

fn detector_address() -> Address {
  Address::new("cluster-test", "127.0.0.1", 0)
}

#[test]
fn poll_error_policy_keeps_cluster_provider_failure_non_fatal() {
  let provider_error = MembershipCoordinatorError::ClusterProvider(ClusterProviderError::down("down failed"));
  assert!(should_continue_after_poll_error(&provider_error));
  assert!(!should_continue_after_poll_error(&MembershipCoordinatorError::NotStarted));
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
  assert!(gossiper.stop().is_ok());
}
