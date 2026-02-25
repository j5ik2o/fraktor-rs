#![allow(clippy::print_stdout)]

//! Membership/Gossip failure detection demo (no_std core).
//!
//! This example drives MembershipCoordinatorGeneric to produce Suspect/Dead transitions,
//! and shows quorum-style branching with topology/metrics linkage.
//!
//! Run:
//! ```bash
//! cargo run -p fraktor-cluster-rs --example membership_gossip_failure_no_std --features test-support
//! ```

#[cfg(not(feature = "test-support"))]
compile_error!("membership_gossip_failure_no_std には --features test-support が必要です。");

use core::time::Duration;

use fraktor_actor_rs::core::{
  event::stream::{EventStreamEvent, EventStreamSharedGeneric, EventStreamSubscriber, subscriber_handle},
  messaging::AnyMessageGeneric,
};
use fraktor_cluster_rs::core::{
  ClusterCore, ClusterEvent, ClusterExtensionConfig, ClusterProviderShared,
  cluster_provider::NoopClusterProvider,
  grain::KindRegistry,
  identity::{IdentityLookupShared, NoopIdentityLookup},
  membership::{
    GossipOutbound, GossiperShared, MembershipCoordinatorConfig, MembershipCoordinatorGeneric,
    MembershipCoordinatorOutcome, MembershipDelta, MembershipSnapshot, MembershipTable, NodeStatus, NoopGossiper,
  },
  placement::ActivatedKind,
  pub_sub::{ClusterPubSubShared, NoopClusterPubSub},
};
use fraktor_remote_rs::core::{
  BlockListProvider,
  failure_detector::{
    DefaultFailureDetectorRegistry,
    phi_failure_detector::{PhiFailureDetector, PhiFailureDetectorConfig},
  },
};
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
      | _ => println!("[observer][event] {cluster_event:?}"),
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
    let threshold = config.phi_threshold;
    let cluster_config = ClusterExtensionConfig::new()
      .with_advertised_address(authority)
      .with_app_version("1.0.0")
      .with_roles(vec![String::from("member")]);
    let registry = DefaultFailureDetectorRegistry::new(Box::new(move || {
      Box::new(PhiFailureDetector::new(PhiFailureDetectorConfig::new(threshold, 10, 1)))
    }));
    let coordinator = MembershipCoordinatorGeneric::<NoStdToolbox>::new(config, cluster_config, table, registry);
    Self { name, authority: authority.to_string(), coordinator, event_stream }
  }

  fn start_member(&mut self) {
    self.coordinator.start_member().expect("start_member");
  }

  fn handle_join(&mut self, node_id: &str, authority: &str, now: TimerInstant) -> Vec<GossipOutbound> {
    let joining_config = ClusterExtensionConfig::new()
      .with_advertised_address(authority)
      .with_app_version("1.0.0")
      .with_roles(vec![String::from("member")]);
    let outcome = self
      .coordinator
      .handle_join(node_id.to_string(), authority.to_string(), &joining_config, now)
      .expect("handle_join");
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
  println!("=== Membership/Gossip Failure Demo (no_std) ===");

  let config = MembershipCoordinatorConfig {
    phi_threshold:          2.0,
    suspect_timeout:        Duration::from_secs(1),
    dead_timeout:           Duration::from_secs(0),
    quarantine_ttl:         Duration::from_secs(5),
    gossip_enabled:         true,
    gossip_interval:        Duration::from_secs(1),
    topology_emit_interval: Duration::from_secs(1),
  };

  let event_stream = EventStreamSharedGeneric::<NoStdToolbox>::default();
  let core = build_cluster_core(event_stream.clone());
  let observer = subscriber_handle(TopologyMetricsObserver::new(core));
  let _subscription = event_stream.subscribe(&observer);

  let mut node_a = DemoNode::new("node-a", "node-a", config.clone(), event_stream.clone());
  let mut node_b = DemoNode::new("node-b", "node-b", config, event_stream.clone());
  node_a.start_member();
  node_b.start_member();

  println!("\n--- Join (node-a/node-b/node-c) ---");
  let t1 = now(1);
  let out = node_a.handle_join("node-a", "node-a", t1);
  deliver_outbounds("node-a", out, &mut node_b, t1);
  let out = node_a.handle_join("node-b", "node-b", t1);
  deliver_outbounds("node-a", out, &mut node_b, t1);
  let out = node_a.handle_join("node-c", "node-c", t1);
  deliver_outbounds("node-a", out, &mut node_b, t1);

  println!("\n--- Heartbeats (baseline) ---");
  let t2 = now(2);
  let out = node_a.handle_heartbeat("node-a", t2);
  deliver_outbounds("node-a", out, &mut node_b, t2);
  let out = node_a.handle_heartbeat("node-b", t2);
  deliver_outbounds("node-a", out, &mut node_b, t2);
  let out = node_a.handle_heartbeat("node-c", t2);
  deliver_outbounds("node-a", out, &mut node_b, t2);

  let t3 = now(3);
  let out = node_a.handle_heartbeat("node-a", t3);
  deliver_outbounds("node-a", out, &mut node_b, t3);
  let out = node_a.handle_heartbeat("node-b", t3);
  deliver_outbounds("node-a", out, &mut node_b, t3);
  let out = node_a.handle_heartbeat("node-c", t3);
  deliver_outbounds("node-a", out, &mut node_b, t3);

  println!("\n--- Quorum check (all up) ---");
  print_quorum("node-a", &node_a.snapshot());
  print_quorum("node-b", &node_b.snapshot());

  println!("\n--- Failure detection (node-c missing) ---");
  let t4 = now(4);
  let out = node_a.handle_heartbeat("node-a", t4);
  deliver_outbounds("node-a", out, &mut node_b, t4);
  let out = node_a.handle_heartbeat("node-b", t4);
  deliver_outbounds("node-a", out, &mut node_b, t4);

  let t5 = now(5);
  let out = node_a.handle_heartbeat("node-a", t5);
  deliver_outbounds("node-a", out, &mut node_b, t5);
  let out = node_a.handle_heartbeat("node-b", t5);
  deliver_outbounds("node-a", out, &mut node_b, t5);

  let t6 = now(6);
  let out = node_a.poll(t6);
  deliver_outbounds("node-a", out, &mut node_b, t6);
  println!("\n--- Quorum check (suspect) ---");
  print_quorum("node-a", &node_a.snapshot());
  print_quorum("node-b", &node_b.snapshot());

  let t7 = now(7);
  let out = node_a.handle_heartbeat("node-a", t7);
  deliver_outbounds("node-a", out, &mut node_b, t7);
  let out = node_a.handle_heartbeat("node-b", t7);
  deliver_outbounds("node-a", out, &mut node_b, t7);

  let t8 = now(8);
  let out = node_a.poll(t8);
  deliver_outbounds("node-a", out, &mut node_b, t8);
  println!("\n--- Quorum check (dead) ---");
  print_quorum("node-a", &node_a.snapshot());
  print_quorum("node-b", &node_b.snapshot());

  println!("\n--- Quorum drop (node-b missing) ---");
  let t9 = now(9);
  let out = node_a.handle_heartbeat("node-a", t9);
  deliver_outbounds("node-a", out, &mut node_b, t9);

  let t10 = now(10);
  let out = node_a.poll(t10);
  deliver_outbounds("node-a", out, &mut node_b, t10);
  println!("\n--- Quorum check (node-b suspect) ---");
  print_quorum("node-a", &node_a.snapshot());
  print_quorum("node-b", &node_b.snapshot());

  let t11 = now(11);
  let out = node_a.handle_heartbeat("node-a", t11);
  deliver_outbounds("node-a", out, &mut node_b, t11);

  let t12 = now(12);
  let out = node_a.poll(t12);
  deliver_outbounds("node-a", out, &mut node_b, t12);
  println!("\n--- Quorum check (node-b dead) ---");
  print_quorum("node-a", &node_a.snapshot());
  print_quorum("node-b", &node_b.snapshot());

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

fn print_quorum(label: &str, snapshot: &MembershipSnapshot) {
  let total = snapshot.entries.len();
  let up_count = snapshot.entries.iter().filter(|entry| entry.status == NodeStatus::Up).count();
  let required = total / 2 + 1;
  let ok = up_count >= required;
  println!("[quorum][{label}] up={up_count} total={total} required={required} ok={ok}");
}
