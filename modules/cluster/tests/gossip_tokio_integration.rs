#![cfg(feature = "std")]

use core::time::Duration;
use std::sync::{Arc, Mutex};

use fraktor_actor_rs::core::event::stream::{
  EventStreamEvent, EventStreamSharedGeneric, EventStreamSubscriber, subscriber_handle,
};
use fraktor_cluster_rs::{
  core::{
    ClusterEvent, ClusterExtensionConfig,
    membership::{
      GossipOutbound, GossipTransport, Gossiper, MembershipCoordinatorConfig, MembershipCoordinatorGeneric,
      MembershipCoordinatorSharedGeneric, MembershipDelta, MembershipTable, MembershipVersion, NodeRecord, NodeStatus,
    },
  },
  std::{TokioGossipTransport, TokioGossipTransportConfig, TokioGossiper, TokioGossiperConfig},
};
use fraktor_remote_rs::core::failure_detector::{
  DefaultFailureDetectorRegistry,
  phi_failure_detector::{PhiFailureDetector, PhiFailureDetectorConfig},
};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

struct EventSink {
  events: Arc<Mutex<Vec<ClusterEvent>>>,
}

impl EventStreamSubscriber<StdToolbox> for EventSink {
  fn on_event(&mut self, event: &EventStreamEvent<StdToolbox>) {
    let EventStreamEvent::Extension { name, payload } = event else {
      return;
    };
    if name != "cluster" {
      return;
    }
    if let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>() {
      self.events.lock().expect("events lock").push(cluster_event.clone());
    }
  }
}

fn build_coordinator() -> MembershipCoordinatorSharedGeneric<StdToolbox> {
  let config = MembershipCoordinatorConfig {
    phi_threshold:          1.0,
    suspect_timeout:        Duration::from_secs(1),
    dead_timeout:           Duration::from_secs(1),
    quarantine_ttl:         Duration::from_secs(1),
    gossip_enabled:         true,
    gossip_interval:        Duration::from_millis(20),
    topology_emit_interval: Duration::from_millis(20),
  };
  let table = MembershipTable::new(3);
  let threshold = config.phi_threshold;
  let registry = DefaultFailureDetectorRegistry::new(Box::new(move || {
    Box::new(PhiFailureDetector::new(PhiFailureDetectorConfig::new(threshold, 10, 1)))
  }));
  let cluster_config = ClusterExtensionConfig::new()
    .with_advertised_address("127.0.0.1:22110")
    .with_app_version("1.0.0")
    .with_roles(vec![String::from("member")]);
  let mut coordinator =
    MembershipCoordinatorGeneric::<StdToolbox>::new(config, cluster_config, table, registry);
  coordinator.start_member().expect("start_member");
  MembershipCoordinatorSharedGeneric::new(coordinator)
}

fn join_delta(authority: &str) -> MembershipDelta {
  let record = NodeRecord::new(
    String::from("node-a"),
    authority.to_string(),
    NodeStatus::Up,
    MembershipVersion::new(1),
    String::from("1.0.0"),
    vec![String::from("member")],
  );
  MembershipDelta::new(MembershipVersion::new(0), MembershipVersion::new(1), vec![record])
}

#[tokio::test]
async fn gossip_delta_triggers_topology_update() {
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let captured = Arc::new(Mutex::new(Vec::new()));
  let subscriber = subscriber_handle(EventSink { events: captured.clone() });
  let _subscription = event_stream.subscribe(&subscriber);

  let coordinator = build_coordinator();
  let transport_b = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8),
    tokio::runtime::Handle::current(),
  )
  .expect("transport bind");
  let target_b = transport_b.local_addr().to_string();
  let mut gossiper = TokioGossiper::new(
    TokioGossiperConfig::new(Duration::from_millis(20), Duration::from_millis(20)),
    coordinator,
    transport_b,
    event_stream.clone(),
    tokio::runtime::Handle::current(),
  );
  gossiper.start().expect("start");

  let mut transport_a = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:0"), 1024, 8),
    tokio::runtime::Handle::current(),
  )
  .expect("transport bind");
  let local_a = transport_a.local_addr().to_string();
  let outbound = GossipOutbound::new(target_b, join_delta(&local_a));
  transport_a.send(outbound).expect("send");

  tokio::time::sleep(Duration::from_millis(120)).await;

  let events = captured.lock().expect("events lock").clone();
  assert!(events.iter().any(|event| matches!(event, ClusterEvent::TopologyUpdated { .. })));

  gossiper.stop().expect("stop");
}
