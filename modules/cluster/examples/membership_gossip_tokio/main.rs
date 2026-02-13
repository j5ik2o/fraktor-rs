#![allow(clippy::print_stdout)]

//! Membership/Gossip Tokio demo.
//!
//! Run:
//! ```bash
//! cargo run -p fraktor-cluster-rs --example membership_gossip_tokio --features std
//! ```

#[cfg(not(feature = "std"))]
compile_error!("membership_gossip_tokio には --features std が必要です。");

use core::time::Duration;

use fraktor_actor_rs::core::event::stream::{
  EventStreamEvent, EventStreamSharedGeneric, EventStreamSubscriber, subscriber_handle,
};
use fraktor_cluster_rs::{
  core::{
    ClusterEvent, GossipOutbound, GossipTransport, Gossiper, MembershipCoordinatorConfig, MembershipCoordinatorGeneric,
    MembershipCoordinatorSharedGeneric, MembershipDelta, MembershipTable, MembershipVersion, NodeRecord, NodeStatus,
  },
  std::{TokioGossipTransport, TokioGossipTransportConfig, TokioGossiper, TokioGossiperConfig},
};
use fraktor_remote_rs::core::failure_detector::{PhiFailureDetector, PhiFailureDetectorConfig};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

struct EventPrinter {
  label: &'static str,
}

impl EventStreamSubscriber<StdToolbox> for EventPrinter {
  fn on_event(&mut self, event: &EventStreamEvent<StdToolbox>) {
    let EventStreamEvent::Extension { name, payload } = event else {
      return;
    };
    if name != "cluster" {
      return;
    }
    if let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>() {
      println!("[{label}] {cluster_event:?}", label = self.label);
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
    gossip_interval:        Duration::from_millis(50),
    topology_emit_interval: Duration::from_millis(50),
  };
  let table = MembershipTable::new(3);
  let detector = PhiFailureDetector::new(PhiFailureDetectorConfig::new(config.phi_threshold, 10, 1));
  let mut coordinator = MembershipCoordinatorGeneric::<StdToolbox>::new(config, table, detector);
  coordinator.start_member().expect("start_member");
  MembershipCoordinatorSharedGeneric::new(coordinator)
}

fn delta_for(node_id: &str, authority: &str, status: NodeStatus, version: u64) -> MembershipDelta {
  let record = NodeRecord::new(node_id.to_string(), authority.to_string(), status, MembershipVersion::new(version));
  MembershipDelta::new(MembershipVersion::new(version.saturating_sub(1)), MembershipVersion::new(version), vec![record])
}

#[tokio::main]
async fn main() {
  println!("=== Membership/Gossip Tokio Demo ===");

  let addr_a = "127.0.0.1:22110";
  let addr_b = "127.0.0.1:22111";

  let event_stream_a = EventStreamSharedGeneric::<StdToolbox>::default();
  let event_stream_b = EventStreamSharedGeneric::<StdToolbox>::default();

  let _sub_a = event_stream_a.subscribe(&subscriber_handle(EventPrinter { label: "node-a" }));
  let _sub_b = event_stream_b.subscribe(&subscriber_handle(EventPrinter { label: "node-b" }));

  let coordinator_a = build_coordinator();
  let coordinator_b = build_coordinator();

  let transport_a = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(addr_a.to_string(), 1024, 16),
    tokio::runtime::Handle::current(),
  )
  .expect("transport bind");
  let transport_b = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(addr_b.to_string(), 1024, 16),
    tokio::runtime::Handle::current(),
  )
  .expect("transport bind");

  let mut gossiper_a = TokioGossiper::new(
    TokioGossiperConfig::new(Duration::from_millis(50), Duration::from_millis(50)),
    coordinator_a,
    transport_a,
    event_stream_a,
    tokio::runtime::Handle::current(),
  );
  let mut gossiper_b = TokioGossiper::new(
    TokioGossiperConfig::new(Duration::from_millis(50), Duration::from_millis(50)),
    coordinator_b,
    transport_b,
    event_stream_b,
    tokio::runtime::Handle::current(),
  );

  gossiper_a.start().expect("start a");
  gossiper_b.start().expect("start b");

  let mut injector = TokioGossipTransport::bind(
    TokioGossipTransportConfig::new(String::from("127.0.0.1:22112"), 1024, 16),
    tokio::runtime::Handle::current(),
  )
  .expect("injector bind");

  println!("--- Join ---");
  let join_a = delta_for("node-a", addr_a, NodeStatus::Up, 1);
  let join_b = delta_for("node-b", addr_b, NodeStatus::Up, 1);
  injector.send(GossipOutbound::new(addr_a.to_string(), join_b.clone())).expect("send join");
  injector.send(GossipOutbound::new(addr_b.to_string(), join_a.clone())).expect("send join");

  tokio::time::sleep(Duration::from_millis(200)).await;

  println!("--- Leave ---");
  let leave_b = delta_for("node-b", addr_b, NodeStatus::Removed, 2);
  injector.send(GossipOutbound::new(addr_a.to_string(), leave_b)).expect("send leave");

  tokio::time::sleep(Duration::from_millis(200)).await;

  gossiper_a.stop().expect("stop a");
  gossiper_b.stop().expect("stop b");

  println!("=== Demo complete ===");
}
