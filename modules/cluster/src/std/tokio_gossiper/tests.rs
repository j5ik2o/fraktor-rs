use core::time::Duration;

use fraktor_actor_rs::core::event::stream::EventStreamSharedGeneric;
use fraktor_remote_rs::core::failure_detector::{
  DefaultFailureDetectorRegistry,
  phi_failure_detector::{PhiFailureDetector, PhiFailureDetectorConfig},
};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::{
  core::membership::{
    Gossiper, MembershipCoordinatorConfig, MembershipCoordinatorGeneric, MembershipCoordinatorSharedGeneric,
    MembershipTable,
  },
  std::{TokioGossipTransport, TokioGossipTransportConfig, TokioGossiper, TokioGossiperConfig},
};

fn build_coordinator() -> MembershipCoordinatorSharedGeneric<StdToolbox> {
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
    Box::new(PhiFailureDetector::new(PhiFailureDetectorConfig::new(threshold, 10, 1)))
  }));
  let mut coordinator = MembershipCoordinatorGeneric::<StdToolbox>::new(config, table, registry);
  coordinator.start_member().expect("start_member");
  MembershipCoordinatorSharedGeneric::new(coordinator)
}

#[tokio::test]
async fn start_then_stop_is_ok() {
  let config = TokioGossiperConfig::new(Duration::from_millis(50), Duration::from_millis(50));
  let coordinator = build_coordinator();
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let transport = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8),
    tokio::runtime::Handle::current(),
  )
  .expect("transport bind");

  let mut gossiper =
    TokioGossiper::new(config, coordinator, transport, event_stream, tokio::runtime::Handle::current());
  assert!(gossiper.start().is_ok());
  assert!(gossiper.stop().is_ok());
}

#[tokio::test]
async fn stop_without_start_returns_err() {
  let config = TokioGossiperConfig::new(Duration::from_millis(50), Duration::from_millis(50));
  let coordinator = build_coordinator();
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let transport = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8),
    tokio::runtime::Handle::current(),
  )
  .expect("transport bind");

  let mut gossiper =
    TokioGossiper::new(config, coordinator, transport, event_stream, tokio::runtime::Handle::current());
  assert!(gossiper.stop().is_err());
}

#[tokio::test]
async fn start_twice_returns_err() {
  let config = TokioGossiperConfig::new(Duration::from_millis(50), Duration::from_millis(50));
  let coordinator = build_coordinator();
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let transport = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8),
    tokio::runtime::Handle::current(),
  )
  .expect("transport bind");

  let mut gossiper =
    TokioGossiper::new(config, coordinator, transport, event_stream, tokio::runtime::Handle::current());
  assert!(gossiper.start().is_ok());
  assert!(gossiper.start().is_err());
  let _ = gossiper.stop();
}
