#![allow(clippy::print_stdout)]

//! Membership/Gossip topology demo (std driver).
//!
//! This example uses MembershipCoordinatorDriverGeneric with an in-memory
//! transport. Cluster events are published to EventStream and applied to
//! ClusterCore to show metrics linkage.
//!
//! Run:
//! ```bash
//! cargo run -p fraktor-cluster-rs --example membership_gossip_topology_std --features std
//! ```

#[cfg(not(feature = "std"))]
compile_error!("membership_gossip_topology_std には --features std が必要です。");

use core::time::Duration;
use std::{
  collections::HashMap,
  sync::{Arc, Mutex},
};

use fraktor_actor_rs::core::event::stream::{
  EventStreamEvent, EventStreamSharedGeneric, EventStreamSubscriber, subscriber_handle,
};
use fraktor_cluster_rs::{
  core::{
    ClusterCore, ClusterEvent, ClusterExtensionConfig, ClusterProviderShared,
    cluster_provider::NoopClusterProvider,
    downing_provider::{DowningProvider, NoopDowningProvider},
    grain::KindRegistry,
    identity::{IdentityLookupShared, NoopIdentityLookup},
    membership::{
      GossipOutbound, GossipTransport, GossipTransportError, GossiperShared, MembershipCoordinatorConfig,
      MembershipCoordinatorGeneric, MembershipCoordinatorSharedGeneric, MembershipDelta, MembershipSnapshot,
      MembershipTable, NoopGossiper,
    },
    placement::ActivatedKind,
    pub_sub::{ClusterPubSubShared, NoopClusterPubSub},
  },
  std::MembershipCoordinatorDriverGeneric,
};
use fraktor_remote_rs::core::{
  BlockListProvider,
  failure_detector::{
    DefaultFailureDetectorRegistry,
    phi_failure_detector::{PhiFailureDetector, PhiFailureDetectorConfig},
  },
};
use fraktor_utils_rs::{
  core::{
    sync::{ArcShared, SharedAccess},
    time::TimerInstant,
  },
  std::runtime_toolbox::{StdMutex, StdToolbox},
};

struct DemoBlockListProvider;

impl BlockListProvider for DemoBlockListProvider {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

struct ClusterEventObserver {
  core: ClusterCore<StdToolbox>,
}

impl ClusterEventObserver {
  fn new(core: ClusterCore<StdToolbox>) -> Self {
    Self { core }
  }
}

impl EventStreamSubscriber<StdToolbox> for ClusterEventObserver {
  fn on_event(&mut self, event: &EventStreamEvent<StdToolbox>) {
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

struct InMemoryBus {
  inbox: HashMap<String, Vec<(String, MembershipDelta)>>,
}

impl InMemoryBus {
  fn new() -> Self {
    Self { inbox: HashMap::new() }
  }

  fn push(&mut self, target: String, sender: String, delta: MembershipDelta) {
    self.inbox.entry(target).or_default().push((sender, delta));
  }

  fn drain(&mut self, target: &str) -> Vec<(String, MembershipDelta)> {
    self.inbox.remove(target).unwrap_or_default()
  }
}

struct DemoTransport {
  node: String,
  bus:  Arc<Mutex<InMemoryBus>>,
}

impl DemoTransport {
  fn new(node: String, bus: Arc<Mutex<InMemoryBus>>) -> Self {
    Self { node, bus }
  }
}

impl GossipTransport for DemoTransport {
  fn send(&mut self, outbound: GossipOutbound) -> Result<(), GossipTransportError> {
    let mut bus = self.bus.lock().expect("bus lock");
    bus.push(outbound.target, self.node.clone(), outbound.delta);
    Ok(())
  }

  fn poll_deltas(&mut self) -> Vec<(String, MembershipDelta)> {
    let mut bus = self.bus.lock().expect("bus lock");
    bus.drain(&self.node)
  }
}

struct DemoNode {
  name:   &'static str,
  driver: MembershipCoordinatorDriverGeneric<StdToolbox, DemoTransport>,
}

impl DemoNode {
  fn new(
    name: &'static str,
    authority: &str,
    config: MembershipCoordinatorConfig,
    bus: Arc<Mutex<InMemoryBus>>,
    event_stream: EventStreamSharedGeneric<StdToolbox>,
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
    let mut coordinator = MembershipCoordinatorGeneric::<StdToolbox>::new(config, cluster_config, table, registry);
    coordinator.start_member().expect("start_member");
    let shared = MembershipCoordinatorSharedGeneric::new(coordinator);
    let transport = DemoTransport::new(authority.to_string(), bus);
    let driver = MembershipCoordinatorDriverGeneric::new(shared, transport, event_stream);
    Self { name, driver }
  }

  fn handle_join(&mut self, node_id: &str, authority: &str, now: TimerInstant) {
    println!("[{}][join] {} -> {}", self.name, node_id, authority);
    let joining_config = ClusterExtensionConfig::new()
      .with_advertised_address(authority)
      .with_app_version("1.0.0")
      .with_roles(vec![String::from("member")]);
    self.driver.handle_join(node_id, authority, &joining_config, now).expect("handle_join");
  }

  fn handle_heartbeat(&mut self, authority: &str, now: TimerInstant) {
    println!("[{}][heartbeat] {authority}", self.name);
    self.driver.handle_heartbeat(authority, now).expect("handle_heartbeat");
  }

  fn poll_gossip(&mut self, now: TimerInstant) {
    self.driver.handle_gossip_deltas(now).expect("handle_gossip_deltas");
  }

  fn poll(&mut self, now: TimerInstant) {
    self.driver.poll(now).expect("poll");
  }

  fn snapshot(&self) -> MembershipSnapshot {
    self.driver.coordinator().with_read(|coordinator| coordinator.snapshot())
  }
}

fn main() {
  println!("=== Membership/Gossip Topology Demo (std) ===");

  let config = MembershipCoordinatorConfig {
    phi_threshold:          1.0,
    suspect_timeout:        Duration::from_secs(2),
    dead_timeout:           Duration::from_secs(0),
    quarantine_ttl:         Duration::from_secs(5),
    gossip_enabled:         true,
    gossip_interval:        Duration::from_secs(1),
    topology_emit_interval: Duration::from_secs(2),
  };

  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let core = build_cluster_core(event_stream.clone());
  let observer = subscriber_handle(ClusterEventObserver::new(core));
  let _subscription = event_stream.subscribe(&observer);

  let bus = Arc::new(Mutex::new(InMemoryBus::new()));
  let mut node_a = DemoNode::new("node-a", "node-a", config.clone(), bus.clone(), event_stream.clone());
  let mut node_b = DemoNode::new("node-b", "node-b", config, bus.clone(), event_stream.clone());

  println!("\n--- Join ---");
  let t1 = now(1);
  node_a.handle_join("node-a", "node-a", t1);
  node_a.handle_join("node-b", "node-b", t1);
  node_b.poll_gossip(t1);

  println!("\n--- Heartbeat ---");
  let t2 = now(2);
  node_a.handle_heartbeat("node-a", t2);
  node_a.handle_heartbeat("node-b", t2);
  node_b.poll_gossip(t2);

  node_b.handle_heartbeat("node-b", t2);
  node_b.handle_heartbeat("node-a", t2);
  node_a.poll_gossip(t2);

  println!("\n--- Topology poll ---");
  node_a.poll(t1);
  node_b.poll(t1);
  let t3 = now(3);
  node_a.poll(t3);
  node_b.poll(t3);

  print_snapshot("node-a", &node_a.snapshot());
  print_snapshot("node-b", &node_b.snapshot());
  print_consensus(&node_a.snapshot(), &node_b.snapshot());

  println!("\n=== Demo complete ===");
}

fn build_cluster_core(event_stream: EventStreamSharedGeneric<StdToolbox>) -> ClusterCore<StdToolbox> {
  let config = ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true);
  let provider = ClusterProviderShared::new(Box::new(NoopClusterProvider::new()));
  let block_list_provider: ArcShared<dyn BlockListProvider> = ArcShared::new(DemoBlockListProvider);
  let gossiper = GossiperShared::new(Box::new(NoopGossiper::new()));
  let downing_provider: ArcShared<StdMutex<Box<dyn DowningProvider>>> =
    ArcShared::new(StdMutex::new(Box::new(NoopDowningProvider::new())));
  let pub_sub = ClusterPubSubShared::new(Box::new(NoopClusterPubSub::new()));
  let identity_lookup = IdentityLookupShared::new(Box::new(NoopIdentityLookup::new()));
  let kind_registry = KindRegistry::new();

  let mut core = ClusterCore::new(
    &config,
    provider,
    block_list_provider,
    event_stream,
    downing_provider,
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

fn print_snapshot(label: &str, snapshot: &MembershipSnapshot) {
  println!("[snapshot][{label}] version={} entries={:?}", snapshot.version.value(), snapshot.entries);
}

fn print_consensus(left: &MembershipSnapshot, right: &MembershipSnapshot) {
  let same_version = left.version == right.version;
  let same_entries = left.entries == right.entries;
  println!("[consensus] version_match={} entries_match={}", same_version, same_entries);
}
