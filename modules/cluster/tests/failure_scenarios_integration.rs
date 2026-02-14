#![cfg(feature = "std")]

use core::time::Duration;
use std::{
  collections::{BTreeSet, HashMap},
  sync::{Arc, Mutex},
};

use fraktor_actor_rs::core::event::stream::EventStreamSharedGeneric;
use fraktor_cluster_rs::{
  core::membership::{
    GossipOutbound, GossipTransport, GossipTransportError, MembershipCoordinatorConfig, MembershipCoordinatorGeneric,
    MembershipCoordinatorSharedGeneric, MembershipDelta, MembershipSnapshot, MembershipTable, NodeStatus,
  },
  std::MembershipCoordinatorDriverGeneric,
};
use fraktor_remote_rs::core::failure_detector::phi_failure_detector::{PhiFailureDetector, PhiFailureDetectorConfig};
use fraktor_utils_rs::{
  core::{sync::SharedAccess, time::TimerInstant},
  std::runtime_toolbox::StdToolbox,
};

struct InMemoryBus {
  inbox:          HashMap<String, Vec<(String, MembershipDelta)>>,
  blocked_routes: BTreeSet<(String, String)>,
}

impl InMemoryBus {
  fn new() -> Self {
    Self { inbox: HashMap::new(), blocked_routes: BTreeSet::new() }
  }

  fn set_route_blocked(&mut self, source: &str, target: &str, blocked: bool) {
    let route = (source.to_string(), target.to_string());
    if blocked {
      self.blocked_routes.insert(route);
    } else {
      self.blocked_routes.remove(&route);
    }
  }

  fn push(&mut self, target: String, sender: String, delta: MembershipDelta) {
    if self.blocked_routes.contains(&(sender.clone(), target.clone())) {
      return;
    }
    self.inbox.entry(target).or_default().push((sender, delta));
  }

  fn drain(&mut self, target: &str) -> Vec<(String, MembershipDelta)> {
    self.inbox.remove(target).unwrap_or_default()
  }

  fn pending_total(&self) -> usize {
    self.inbox.values().map(Vec::len).sum()
  }
}

struct DemoTransport {
  authority: String,
  bus:       Arc<Mutex<InMemoryBus>>,
}

impl DemoTransport {
  fn new(authority: &str, bus: Arc<Mutex<InMemoryBus>>) -> Self {
    Self { authority: authority.to_string(), bus }
  }
}

impl GossipTransport for DemoTransport {
  fn send(&mut self, outbound: GossipOutbound) -> Result<(), GossipTransportError> {
    let mut bus = self.bus.lock().expect("bus lock");
    bus.push(outbound.target, self.authority.clone(), outbound.delta);
    Ok(())
  }

  fn poll_deltas(&mut self) -> Vec<(String, MembershipDelta)> {
    let mut bus = self.bus.lock().expect("bus lock");
    bus.drain(&self.authority)
  }
}

struct DemoNode {
  driver: MembershipCoordinatorDriverGeneric<StdToolbox, DemoTransport>,
}

impl DemoNode {
  fn new(
    authority: &str,
    config: MembershipCoordinatorConfig,
    bus: Arc<Mutex<InMemoryBus>>,
    event_stream: EventStreamSharedGeneric<StdToolbox>,
  ) -> Self {
    let table = MembershipTable::new(3);
    let detector = PhiFailureDetector::new(PhiFailureDetectorConfig::new(config.phi_threshold, 10, 1));
    let mut coordinator = MembershipCoordinatorGeneric::<StdToolbox>::new(config, table, detector);
    coordinator.start_member().expect("start_member");
    let shared = MembershipCoordinatorSharedGeneric::new(coordinator);
    let transport = DemoTransport::new(authority, bus);
    let driver = MembershipCoordinatorDriverGeneric::new(shared, transport, event_stream);
    Self { driver }
  }

  fn handle_join(&mut self, node_id: &str, authority: &str, now: TimerInstant) {
    self.driver.handle_join(node_id, authority, now).expect("handle_join");
  }

  fn handle_heartbeat(&mut self, authority: &str, now: TimerInstant) {
    self.driver.handle_heartbeat(authority, now).expect("handle_heartbeat");
  }

  fn poll(&mut self, now: TimerInstant) {
    self.driver.poll(now).expect("poll");
  }

  fn poll_gossip(&mut self, now: TimerInstant) {
    self.driver.handle_gossip_deltas(now).expect("handle_gossip_deltas");
  }

  fn snapshot(&self) -> MembershipSnapshot {
    self.driver.coordinator().with_read(|coordinator| coordinator.snapshot())
  }

  fn status_of(&self, authority: &str) -> Option<NodeStatus> {
    self.snapshot().entries.into_iter().find(|record| record.authority == authority).map(|record| record.status)
  }
}

fn config() -> MembershipCoordinatorConfig {
  MembershipCoordinatorConfig {
    phi_threshold:          2.0,
    suspect_timeout:        Duration::from_secs(1),
    dead_timeout:           Duration::from_secs(0),
    quarantine_ttl:         Duration::from_secs(5),
    gossip_enabled:         true,
    gossip_interval:        Duration::from_secs(1),
    topology_emit_interval: Duration::from_secs(1),
  }
}

fn config_with_suspect_timeout(timeout: Duration) -> MembershipCoordinatorConfig {
  let mut config = config();
  config.suspect_timeout = timeout;
  config
}

fn now(ticks: u64) -> TimerInstant {
  TimerInstant::from_ticks(ticks, Duration::from_secs(1))
}

fn set_partition(bus: &Arc<Mutex<InMemoryBus>>, left: &str, right: &str, blocked: bool) {
  let mut guard = bus.lock().expect("bus lock");
  guard.set_route_blocked(left, right, blocked);
  guard.set_route_blocked(right, left, blocked);
}

fn assert_status(node: &DemoNode, authority: &str, expected: NodeStatus) {
  let actual = node.status_of(authority);
  assert_eq!(actual, Some(expected), "authority={authority}");
}

fn pending_messages(bus: &Arc<Mutex<InMemoryBus>>) -> usize {
  bus.lock().expect("bus lock").pending_total()
}

fn assert_status_eventually(
  authority: &str,
  expected: NodeStatus,
  max_steps: usize,
  mut step_and_observe: impl FnMut() -> Option<NodeStatus>,
) {
  let mut actual = None;
  for _ in 0..max_steps {
    actual = step_and_observe();
    if actual == Some(expected) {
      return;
    }
  }
  panic!("status did not converge: authority={authority} expected={expected:?} actual={actual:?}");
}

#[test]
fn node_down_is_marked_dead_after_failure_detection() {
  let bus = Arc::new(Mutex::new(InMemoryBus::new()));
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let mut node_a = DemoNode::new("node-a", config(), bus.clone(), event_stream.clone());
  let mut node_b = DemoNode::new("node-b", config(), bus, event_stream);

  let t1 = now(1);
  node_a.handle_join("node-a", "node-a", t1);
  node_a.handle_join("node-b", "node-b", t1);
  node_a.handle_join("node-c", "node-c", t1);
  node_b.poll_gossip(t1);

  let t2 = now(2);
  node_a.handle_heartbeat("node-a", t2);
  node_a.handle_heartbeat("node-b", t2);
  node_a.handle_heartbeat("node-c", t2);
  node_b.poll_gossip(t2);

  let t3 = now(3);
  node_a.handle_heartbeat("node-a", t3);
  node_a.handle_heartbeat("node-b", t3);
  node_a.handle_heartbeat("node-c", t3);
  node_b.poll_gossip(t3);

  let t4 = now(4);
  node_a.handle_heartbeat("node-a", t4);
  node_a.handle_heartbeat("node-b", t4);
  node_b.poll_gossip(t4);

  let t5 = now(5);
  node_a.handle_heartbeat("node-a", t5);
  node_a.handle_heartbeat("node-b", t5);
  node_b.poll_gossip(t5);

  let t6 = now(6);
  node_a.poll(t6);
  node_b.poll_gossip(t6);
  let mut suspect_tick_a = 6_u64;
  assert_status_eventually("node-c", NodeStatus::Suspect, 3, || {
    node_a.poll(now(suspect_tick_a));
    node_b.poll_gossip(now(suspect_tick_a));
    suspect_tick_a += 1;
    node_a.status_of("node-c")
  });
  let mut suspect_tick_b = 6_u64;
  assert_status_eventually("node-c", NodeStatus::Suspect, 3, || {
    node_a.poll(now(suspect_tick_b));
    node_b.poll_gossip(now(suspect_tick_b));
    suspect_tick_b += 1;
    node_b.status_of("node-c")
  });

  let mut tick = 7_u64;
  assert_status_eventually("node-c", NodeStatus::Dead, 4, || {
    node_a.poll(now(tick));
    node_b.poll_gossip(now(tick));
    tick += 1;
    node_a.status_of("node-c")
  });
  assert_status_eventually("node-c", NodeStatus::Dead, 4, || {
    node_a.poll(now(tick));
    node_b.poll_gossip(now(tick));
    tick += 1;
    node_b.status_of("node-c")
  });
}

#[test]
fn network_partition_marks_suspect_and_recovers_after_heal() {
  let bus = Arc::new(Mutex::new(InMemoryBus::new()));
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let config = config_with_suspect_timeout(Duration::from_secs(4));
  let mut node_a = DemoNode::new("node-a", config.clone(), bus.clone(), event_stream.clone());
  let mut node_b = DemoNode::new("node-b", config, bus.clone(), event_stream);

  let t1 = now(1);
  node_a.handle_join("node-a", "node-a", t1);
  node_a.handle_join("node-b", "node-b", t1);
  node_b.poll_gossip(t1);

  let t2 = now(2);
  node_a.handle_heartbeat("node-a", t2);
  node_a.handle_heartbeat("node-b", t2);
  node_a.poll_gossip(t2);
  node_b.poll_gossip(t2);
  node_b.handle_heartbeat("node-a", t2);
  node_b.handle_heartbeat("node-b", t2);

  let t3 = now(3);
  node_a.handle_heartbeat("node-a", t3);
  node_a.handle_heartbeat("node-b", t3);
  node_b.handle_heartbeat("node-a", t3);
  node_b.handle_heartbeat("node-b", t3);
  node_a.poll_gossip(t3);
  node_b.poll_gossip(t3);

  set_partition(&bus, "node-a", "node-b", true);

  let t4 = now(4);
  node_a.handle_heartbeat("node-a", t4);
  node_b.handle_heartbeat("node-b", t4);
  node_a.poll(t4);
  node_b.poll(t4);

  let t5 = now(5);
  node_a.handle_heartbeat("node-a", t5);
  node_b.handle_heartbeat("node-b", t5);
  node_a.poll(t5);
  node_b.poll(t5);
  let mut suspect_tick_a = 5_u64;
  assert_status_eventually("node-b", NodeStatus::Suspect, 3, || {
    node_a.poll(now(suspect_tick_a));
    node_b.poll(now(suspect_tick_a));
    suspect_tick_a += 1;
    node_a.status_of("node-b")
  });
  let mut suspect_tick_b = 5_u64;
  assert_status_eventually("node-a", NodeStatus::Suspect, 3, || {
    node_a.poll(now(suspect_tick_b));
    node_b.poll(now(suspect_tick_b));
    suspect_tick_b += 1;
    node_b.status_of("node-a")
  });

  set_partition(&bus, "node-a", "node-b", false);

  // 分断解除だけでは復旧せず、heartbeat が必要であることを明示する。
  let t6 = now(6);
  node_a.poll_gossip(t6);
  node_b.poll_gossip(t6);
  node_a.poll(t6);
  node_b.poll(t6);
  assert_status(&node_a, "node-b", NodeStatus::Suspect);
  assert_status(&node_b, "node-a", NodeStatus::Suspect);

  let t7 = now(7);
  node_a.handle_heartbeat("node-b", t7);
  node_b.handle_heartbeat("node-a", t7);
  assert!(pending_messages(&bus) > 0, "recovery heartbeat should enqueue gossip deltas through transport");
  node_a.poll_gossip(t7);
  node_b.poll_gossip(t7);
  assert_eq!(pending_messages(&bus), 0, "queued gossip deltas should be drained");
  node_a.poll(t7);
  node_b.poll(t7);
  let mut up_tick_a = 7_u64;
  assert_status_eventually("node-b", NodeStatus::Up, 3, || {
    node_a.poll(now(up_tick_a));
    node_b.poll(now(up_tick_a));
    up_tick_a += 1;
    node_a.status_of("node-b")
  });
  let mut up_tick_b = 7_u64;
  assert_status_eventually("node-a", NodeStatus::Up, 3, || {
    node_a.poll(now(up_tick_b));
    node_b.poll(now(up_tick_b));
    up_tick_b += 1;
    node_b.status_of("node-a")
  });
}

#[test]
fn slow_node_transitions_to_suspect_then_returns_up() {
  let bus = Arc::new(Mutex::new(InMemoryBus::new()));
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let mut node_a = DemoNode::new("node-a", config(), bus, event_stream);

  let t1 = now(1);
  node_a.handle_join("node-a", "node-a", t1);
  node_a.handle_join("node-b", "node-b", t1);

  let t2 = now(2);
  node_a.handle_heartbeat("node-a", t2);
  node_a.handle_heartbeat("node-b", t2);

  let t3 = now(3);
  node_a.handle_heartbeat("node-a", t3);
  node_a.handle_heartbeat("node-b", t3);
  node_a.poll(t3);

  let t4 = now(4);
  node_a.handle_heartbeat("node-a", t4);
  node_a.poll(t4);

  let t5 = now(5);
  node_a.handle_heartbeat("node-a", t5);
  node_a.poll(t5);
  let mut suspect_tick = 5_u64;
  assert_status_eventually("node-b", NodeStatus::Suspect, 3, || {
    node_a.poll(now(suspect_tick));
    suspect_tick += 1;
    node_a.status_of("node-b")
  });

  let t6 = now(6);
  node_a.handle_heartbeat("node-b", t6);
  let mut up_tick = 6_u64;
  assert_status_eventually("node-b", NodeStatus::Up, 3, || {
    node_a.poll(now(up_tick));
    up_tick += 1;
    node_a.status_of("node-b")
  });
}
