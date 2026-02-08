//! Membership and gossip topology demo for no_std core.
#![allow(clippy::print_stdout)]

//! Membership/Gossip topology demo (no_std core).
//!
//! This example drives MembershipCoordinatorGeneric directly and publishes cluster
//! events to EventStream. Topology updates are applied to ClusterCore to show
//! metrics linkage.
//!
//! Run:
//! ```bash
//! cargo run -p fraktor-cluster-rs --example membership_gossip_topology_no_std --features test-support
//! ```

#[cfg(not(feature = "test-support"))]
compile_error!("membership_gossip_topology_no_std には --features test-support が必要です。");

use core::time::Duration;

use fraktor_actor_rs::core::{
  event::stream::{EventStreamEvent, EventStreamSharedGeneric, EventStreamSubscriber, subscriber_handle},
  messaging::AnyMessageGeneric,
};
use fraktor_cluster_rs::core::{
  ActivatedKind, ClusterCore, ClusterEvent, ClusterExtensionConfig, ClusterProviderShared, ClusterPubSubShared,
  GossipOutbound, GossiperShared, IdentityLookupShared, KindRegistry, MembershipCoordinatorConfig,
  MembershipCoordinatorGeneric, MembershipCoordinatorOutcome, MembershipDelta, MembershipSnapshot, MembershipTable,
  NoopClusterProvider, NoopClusterPubSub, NoopGossiper, NoopIdentityLookup,
};
use fraktor_remote_rs::core::{BlockListProvider, PhiFailureDetector, PhiFailureDetectorConfig};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared, time::TimerInstant};

struct DemoBlockListProvider;

impl BlockListProvider for DemoBlockListProvider {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

struct TopologyMetricsObserver {
  core: ClusterCore<NoStdToolbox>,
}

impl TopologyMetricsObserver {
  fn new(core: ClusterCore<NoStdToolbox>) -> Self {
    Self { core }
  }
}

impl EventStreamSubscriber<NoStdToolbox> for TopologyMetricsObserver {
  fn on_event(&mut self, event: &EventStreamEvent<NoStdToolbox>) {
    let EventStreamEvent::Extension { name, payload } = event else {
      return;
    };
    if name != "cluster" {
      return;
    }
    let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>() else {
      return;
    };

    match cluster_event {
      | ClusterEvent::TopologyUpdated { update } => {
        if let Err(err) = self.core.apply_topology(update) {
          println!("[observer][metrics] topology apply failed: {err:?}");
          return;
        }
        if let Ok(snapshot) = self.core.metrics() {
          println!("[observer][metrics] members={} virtual_actors={}", snapshot.members(), snapshot.virtual_actors());
        }
      },
      | _ => {
        println!("[observer][event] {cluster_event:?}");
      },
    }
  }
}

struct DemoNode {
  name:         &'static str,
  authority:    String,
  coordinator:  MembershipCoordinatorGeneric<NoStdToolbox>,
  event_stream: EventStreamSharedGeneric<NoStdToolbox>,
}

impl DemoNode {
  fn new(
    name: &'static str,
    authority: &str,
    config: MembershipCoordinatorConfig,
    event_stream: EventStreamSharedGeneric<NoStdToolbox>,
  ) -> Self {
    let table = MembershipTable::new(3);
    let detector = PhiFailureDetector::new(PhiFailureDetectorConfig::new(config.phi_threshold, 10, 1));
    let coordinator = MembershipCoordinatorGeneric::<NoStdToolbox>::new(config, table, detector);
    Self { name, authority: authority.to_string(), coordinator, event_stream }
  }

  fn start_member(&mut self) {
    self.coordinator.start_member().expect("start_member");
  }

  fn handle_join(&mut self, node_id: &str, authority: &str, now: TimerInstant) -> Vec<GossipOutbound> {
    let outcome = self.coordinator.handle_join(node_id.to_string(), authority.to_string(), now).expect("handle_join");
    self.apply_outcome(outcome)
  }

  fn handle_heartbeat(&mut self, authority: &str, now: TimerInstant) -> Vec<GossipOutbound> {
    let outcome = self.coordinator.handle_heartbeat(authority, now).expect("handle_heartbeat");
    self.apply_outcome(outcome)
  }

  fn handle_gossip_delta(&mut self, peer: &str, delta: &MembershipDelta, now: TimerInstant) {
    let outcome = self.coordinator.handle_gossip_delta(peer, delta, now).expect("handle_gossip_delta");
    let _ = self.apply_outcome(outcome);
  }

  fn poll(&mut self, now: TimerInstant) -> Vec<GossipOutbound> {
    let outcome = self.coordinator.poll(now).expect("poll");
    self.apply_outcome(outcome)
  }

  fn snapshot(&self) -> MembershipSnapshot {
    self.coordinator.snapshot()
  }

  fn apply_outcome(&mut self, outcome: MembershipCoordinatorOutcome) -> Vec<GossipOutbound> {
    if let Some(event) = outcome.topology_event {
      self.publish_cluster_event(&event);
    }
    for event in outcome.member_events {
      self.publish_cluster_event(&event);
    }
    for event in outcome.membership_events {
      println!("[{}][membership] {event:?}", self.name);
    }
    for event in outcome.quarantine_events {
      println!("[{}][quarantine] {event:?}", self.name);
    }
    if !outcome.gossip_outbound.is_empty() {
      println!("[{}][gossip] outbound={}", self.name, outcome.gossip_outbound.len());
    }
    outcome.gossip_outbound
  }

  fn publish_cluster_event(&self, event: &ClusterEvent) {
    println!("[{}][cluster] {event:?}", self.name);
    let payload = AnyMessageGeneric::new(event.clone());
    let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }
}

fn main() {
  println!("=== Membership/Gossip Topology Demo (no_std) ===");

  let config = MembershipCoordinatorConfig {
    phi_threshold:          1.0,
    suspect_timeout:        Duration::from_secs(2),
    dead_timeout:           Duration::from_secs(0),
    quarantine_ttl:         Duration::from_secs(5),
    gossip_enabled:         true,
    gossip_interval:        Duration::from_secs(1),
    topology_emit_interval: Duration::from_secs(2),
  };

  let event_stream = EventStreamSharedGeneric::<NoStdToolbox>::default();
  let core = build_cluster_core(event_stream.clone());
  let observer = subscriber_handle(TopologyMetricsObserver::new(core));
  let _subscription = event_stream.subscribe(&observer);

  let mut node_a = DemoNode::new("node-a", "node-a", config.clone(), event_stream.clone());
  let mut node_b = DemoNode::new("node-b", "node-b", config, event_stream.clone());
  node_a.start_member();
  node_b.start_member();

  println!("\n--- Join ---");
  let t1 = now(1);
  let out_a1 = node_a.handle_join("node-a", "node-a", t1);
  let out_a2 = node_a.handle_join("node-b", "node-b", t1);
  deliver_outbounds("node-a", out_a1, &mut node_b, t1);
  deliver_outbounds("node-a", out_a2, &mut node_b, t1);

  println!("\n--- Heartbeat ---");
  let t2 = now(2);
  let out_a = node_a.handle_heartbeat("node-a", t2);
  deliver_outbounds("node-a", out_a, &mut node_b, t2);
  let out_a = node_a.handle_heartbeat("node-b", t2);
  deliver_outbounds("node-a", out_a, &mut node_b, t2);

  let out_b = node_b.handle_heartbeat("node-b", t2);
  deliver_outbounds("node-b", out_b, &mut node_a, t2);
  let out_b = node_b.handle_heartbeat("node-a", t2);
  deliver_outbounds("node-b", out_b, &mut node_a, t2);

  println!("\n--- Topology poll ---");
  let _ = node_a.poll(t1);
  let _ = node_b.poll(t1);
  let t3 = now(3);
  let _ = node_a.poll(t3);
  let _ = node_b.poll(t3);

  print_snapshot("node-a", &node_a.snapshot());
  print_snapshot("node-b", &node_b.snapshot());
  print_consensus(&node_a.snapshot(), &node_b.snapshot());

  println!("\n=== Demo complete ===");
}

fn build_cluster_core(event_stream: EventStreamSharedGeneric<NoStdToolbox>) -> ClusterCore<NoStdToolbox> {
  let config = ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true);
  let provider = ClusterProviderShared::new(Box::new(NoopClusterProvider::new()));
  let block_list_provider: ArcShared<dyn BlockListProvider> = ArcShared::new(DemoBlockListProvider);
  let gossiper = GossiperShared::new(Box::new(NoopGossiper::new()));
  let pub_sub = ClusterPubSubShared::new(Box::new(NoopClusterPubSub::new()));
  let identity_lookup = IdentityLookupShared::new(Box::new(NoopIdentityLookup::new()));
  let kind_registry = KindRegistry::new();

  let mut core = ClusterCore::new(
    &config,
    provider,
    block_list_provider,
    event_stream,
    gossiper,
    pub_sub,
    kind_registry,
    identity_lookup,
  );
  core.setup_member_kinds(vec![ActivatedKind::new("grain")]).expect("setup_member_kinds");
  core.start_member().expect("start_member");
  core
}

fn now(ticks: u64) -> TimerInstant {
  TimerInstant::from_ticks(ticks, Duration::from_secs(1))
}

fn deliver_outbounds(sender: &str, outbounds: Vec<GossipOutbound>, receiver: &mut DemoNode, now: TimerInstant) {
  for outbound in outbounds {
    if outbound.target == receiver.authority {
      receiver.handle_gossip_delta(sender, &outbound.delta, now);
    }
  }
}

fn print_snapshot(label: &str, snapshot: &MembershipSnapshot) {
  println!("[snapshot][{label}] version={} entries={:?}", snapshot.version.value(), snapshot.entries);
}

fn print_consensus(left: &MembershipSnapshot, right: &MembershipSnapshot) {
  let same_version = left.version == right.version;
  let same_entries = left.entries == right.entries;
  println!("[consensus] version_match={} entries_match={}", same_version, same_entries);
}
