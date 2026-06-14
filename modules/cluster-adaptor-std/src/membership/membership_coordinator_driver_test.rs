use core::time::Duration;
use std::{
  collections::{BTreeSet, HashMap},
  sync::{Arc, Mutex},
};

use fraktor_actor_core_kernel_rs::event::stream::EventStreamShared;
use fraktor_cluster_core_kernel_rs::{
  cluster_provider::ClusterProvider,
  downing_provider::{SplitBrainResolverConfig, SplitBrainResolverStrategy},
  extension::{ClusterExtensionConfig, ClusterProviderError, ClusterProviderShared},
  failure_detector::{DefaultFailureDetectorRegistry, FailureDetectorConfig},
  membership::{
    GossipOutbound, GossipTransport, GossipTransportError, MembershipCoordinator, MembershipCoordinatorConfig,
    MembershipCoordinatorError, MembershipCoordinatorShared, MembershipDelta, MembershipSnapshot, MembershipTable,
    NodeStatus,
  },
};
use fraktor_remote_core_rs::address::Address;
use fraktor_utils_core_rs::{sync::SharedAccess, time::TimerInstant};

use super::MembershipCoordinatorDriver;
use crate::{cluster_provider::StdSplitBrainResolverProvider, membership::ConfiguredPhiAccrualDetectorFactory};

impl<TTransport: GossipTransport> MembershipCoordinatorDriver<TTransport> {
  const fn coordinator(&self) -> &MembershipCoordinatorShared {
    &self.coordinator
  }

  fn handle_join(
    &mut self,
    node_id: impl Into<String>,
    authority: impl Into<String>,
    joining_config: &ClusterExtensionConfig,
    now: TimerInstant,
  ) {
    let outcome = self
      .coordinator
      .with_write(|coordinator| coordinator.handle_join(node_id.into(), authority.into(), joining_config, now))
      .expect("handle_join");
    self.apply_outcome(outcome).expect("apply handle_join outcome");
  }

  fn handle_heartbeat(&mut self, authority: &str, now: TimerInstant) {
    let outcome = self
      .coordinator
      .with_write(|coordinator| coordinator.handle_heartbeat(authority, now))
      .expect("handle_heartbeat");
    self.apply_outcome(outcome).expect("apply handle_heartbeat outcome");
  }
}

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
  driver:        MembershipCoordinatorDriver<DemoTransport>,
  phi_threshold: f64,
}

impl DemoNode {
  fn new(
    authority: &str,
    config: MembershipCoordinatorConfig,
    bus: Arc<Mutex<InMemoryBus>>,
    event_stream: EventStreamShared,
  ) -> Self {
    let table = MembershipTable::new(3);
    let phi_threshold = config.phi_threshold;
    let cluster_config = ClusterExtensionConfig::new()
      .with_advertised_address(authority)
      .with_app_version("1.0.0")
      .with_roles(vec![String::from("member")])
      .with_failure_detector_config(FailureDetectorConfig::new().with_phi_threshold(phi_threshold));
    let detector_config = *cluster_config.failure_detector_config();
    let registry = DefaultFailureDetectorRegistry::new(Box::new(move || {
      ConfiguredPhiAccrualDetectorFactory::new(detector_config, detector_address()).create()
    }));
    let mut coordinator = MembershipCoordinator::new(config, cluster_config, table, registry);
    coordinator.start_member().expect("start_member");
    let shared = MembershipCoordinatorShared::new(coordinator);
    let transport = DemoTransport::new(authority, bus);
    let driver = MembershipCoordinatorDriver::new(shared, transport, event_stream);
    Self { driver, phi_threshold }
  }

  fn with_split_brain_resolver_downing(
    mut self,
    local_authority: impl Into<String>,
    cluster_provider: ClusterProviderShared,
  ) -> Self {
    let provider = StdSplitBrainResolverProvider::new(SplitBrainResolverConfig::new(
      Duration::ZERO,
      SplitBrainResolverStrategy::KeepMajority,
      Duration::from_secs(30),
    ));
    self.driver = self.driver.with_split_brain_resolver_downing(provider, local_authority, cluster_provider);
    self
  }

  fn handle_join(&mut self, node_id: &str, authority: &str, now: TimerInstant) {
    let joining_config = ClusterExtensionConfig::new()
      .with_advertised_address(authority)
      .with_app_version("1.0.0")
      .with_roles(vec![String::from("member")])
      .with_failure_detector_config(FailureDetectorConfig::new().with_phi_threshold(self.phi_threshold));
    self.driver.handle_join(node_id, authority, &joining_config, now);
  }

  fn handle_heartbeat(&mut self, authority: &str, now: TimerInstant) {
    self.driver.handle_heartbeat(authority, now);
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

fn detector_address() -> Address {
  Address::new("cluster-test", "127.0.0.1", 0)
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

#[derive(Clone, Default)]
struct RecordingProviderState {
  members: Arc<Mutex<BTreeSet<String>>>,
}

impl RecordingProviderState {
  fn with_members(authorities: &[&str]) -> Self {
    let state = Self::default();
    {
      let mut members = state.members.lock().expect("members lock");
      members.extend(authorities.iter().map(|authority| (*authority).to_string()));
    }
    state
  }

  fn member_count(&self) -> usize {
    self.members.lock().expect("members lock").len()
  }
}

struct RecordingClusterProvider {
  state: RecordingProviderState,
}

impl RecordingClusterProvider {
  fn shared(authorities: &[&str]) -> (ClusterProviderShared, RecordingProviderState) {
    let state = RecordingProviderState::with_members(authorities);
    let provider = Self { state: state.clone() };
    (ClusterProviderShared::new(Box::new(provider)), state)
  }
}

impl ClusterProvider for RecordingClusterProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    self.state.members.lock().expect("members lock").remove(authority);
    Ok(())
  }

  fn join(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    self.state.members.lock().expect("members lock").insert(authority.to_string());
    Ok(())
  }

  fn leave(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    self.state.members.lock().expect("members lock").remove(authority);
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

struct FailingDownClusterProvider;

impl ClusterProvider for FailingDownClusterProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Err(ClusterProviderError::down("down failed"))
  }

  fn join(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn leave(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

struct FailingSelfDownClusterProvider {
  local_authority: String,
}

impl FailingSelfDownClusterProvider {
  fn new(local_authority: impl Into<String>) -> Self {
    Self { local_authority: local_authority.into() }
  }
}

impl ClusterProvider for FailingSelfDownClusterProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    if authority == self.local_authority {
      return Err(ClusterProviderError::down("cannot down self authority"));
    }
    Ok(())
  }

  fn join(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn leave(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

struct FailingAuthorityDownClusterProvider {
  failing_authority: String,
}

impl FailingAuthorityDownClusterProvider {
  fn new(failing_authority: impl Into<String>) -> Self {
    Self { failing_authority: failing_authority.into() }
  }
}

impl ClusterProvider for FailingAuthorityDownClusterProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    if authority == self.failing_authority {
      return Err(ClusterProviderError::down(format!("down failed for {authority}")));
    }
    Ok(())
  }

  fn join(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn leave(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
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
fn failure_detection_keeps_suspect_until_downing_decision() {
  let bus = Arc::new(Mutex::new(InMemoryBus::new()));
  let event_stream = EventStreamShared::default();
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

  for tick in 7..=10 {
    node_a.poll(now(tick));
    node_b.poll_gossip(now(tick));
  }
  assert_status(&node_a, "node-c", NodeStatus::Suspect);
  assert_status(&node_b, "node-c", NodeStatus::Suspect);
}

#[test]
fn split_brain_resolver_downing_downs_unreachable_target_when_attached() {
  let bus = Arc::new(Mutex::new(InMemoryBus::new()));
  let event_stream = EventStreamShared::default();
  let (provider, provider_state) = RecordingClusterProvider::shared(&["node-a", "node-b", "node-c"]);
  let mut node_a =
    DemoNode::new("node-a", config(), bus, event_stream).with_split_brain_resolver_downing("node-a", provider);

  let t1 = now(1);
  node_a.handle_join("node-a", "node-a", t1);
  node_a.handle_join("node-b", "node-b", t1);
  node_a.handle_join("node-c", "node-c", t1);

  let t2 = now(2);
  node_a.handle_heartbeat("node-a", t2);
  node_a.handle_heartbeat("node-b", t2);
  node_a.handle_heartbeat("node-c", t2);

  let t3 = now(3);
  node_a.handle_heartbeat("node-a", t3);
  node_a.handle_heartbeat("node-b", t3);
  node_a.handle_heartbeat("node-c", t3);

  let t4 = now(4);
  node_a.handle_heartbeat("node-a", t4);
  node_a.handle_heartbeat("node-b", t4);

  let t5 = now(5);
  node_a.handle_heartbeat("node-a", t5);
  node_a.handle_heartbeat("node-b", t5);

  assert_eq!(provider_state.member_count(), 3);
  let mut down_tick = 5_u64;
  assert_status_eventually("node-c", NodeStatus::Dead, 3, || {
    node_a.poll(now(down_tick));
    down_tick += 1;
    node_a.status_of("node-c")
  });

  assert_eq!(provider_state.member_count(), 2);
}

#[test]
fn split_brain_resolver_downing_keeps_membership_when_provider_down_fails() {
  let bus = Arc::new(Mutex::new(InMemoryBus::new()));
  let event_stream = EventStreamShared::default();
  let provider = ClusterProviderShared::new(Box::new(FailingDownClusterProvider));
  let mut node_a =
    DemoNode::new("node-a", config(), bus, event_stream).with_split_brain_resolver_downing("node-a", provider);

  let t1 = now(1);
  node_a.handle_join("node-a", "node-a", t1);
  node_a.handle_join("node-b", "node-b", t1);
  node_a.handle_join("node-c", "node-c", t1);

  let t2 = now(2);
  node_a.handle_heartbeat("node-a", t2);
  node_a.handle_heartbeat("node-b", t2);
  node_a.handle_heartbeat("node-c", t2);

  let t3 = now(3);
  node_a.handle_heartbeat("node-a", t3);
  node_a.handle_heartbeat("node-b", t3);
  node_a.handle_heartbeat("node-c", t3);

  let t4 = now(4);
  node_a.handle_heartbeat("node-a", t4);
  node_a.handle_heartbeat("node-b", t4);

  let t5 = now(5);
  node_a.handle_heartbeat("node-a", t5);
  node_a.handle_heartbeat("node-b", t5);

  let err = node_a.driver.poll(now(6)).expect_err("provider down failure");
  assert!(matches!(
    err,
    MembershipCoordinatorError::ClusterProvider(ClusterProviderError::DownFailed(reason))
      if reason == "down failed"
  ));
  assert_status(&node_a, "node-c", NodeStatus::Suspect);
}

#[test]
fn split_brain_resolver_downing_applies_membership_per_successful_provider_target() {
  let bus = Arc::new(Mutex::new(InMemoryBus::new()));
  let event_stream = EventStreamShared::default();
  let cluster_provider = ClusterProviderShared::new(Box::new(FailingAuthorityDownClusterProvider::new("node-c")));
  let provider = StdSplitBrainResolverProvider::new(SplitBrainResolverConfig::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::DownAll,
    Duration::ZERO,
  ));
  let mut node_a = DemoNode::new("node-a", config(), bus, event_stream);
  node_a.driver = node_a.driver.with_split_brain_resolver_downing(provider, "node-a", cluster_provider);

  let t1 = now(1);
  node_a.handle_join("node-a", "node-a", t1);
  node_a.handle_join("node-b", "node-b", t1);
  node_a.handle_join("node-c", "node-c", t1);

  let t2 = now(2);
  node_a.handle_heartbeat("node-a", t2);
  node_a.handle_heartbeat("node-b", t2);
  node_a.handle_heartbeat("node-c", t2);

  let t3 = now(3);
  node_a.handle_heartbeat("node-a", t3);
  node_a.handle_heartbeat("node-b", t3);
  node_a.handle_heartbeat("node-c", t3);

  let t4 = now(4);
  node_a.handle_heartbeat("node-a", t4);

  let t5 = now(5);
  node_a.handle_heartbeat("node-a", t5);

  let mut observed_error = None;
  for tick in 6..=8 {
    match node_a.driver.poll(now(tick)) {
      | Ok(()) => {},
      | Err(error) => {
        observed_error = Some(error);
        break;
      },
    }
  }
  let err = observed_error.expect("provider down failure");
  assert!(matches!(
    err,
    MembershipCoordinatorError::ClusterProvider(ClusterProviderError::DownFailed(reason))
      if reason == "down failed for node-c"
  ));
  assert_status(&node_a, "node-a", NodeStatus::Dead);
  assert_status(&node_a, "node-b", NodeStatus::Dead);
  assert_status(&node_a, "node-c", NodeStatus::Suspect);
}

#[test]
fn split_brain_resolver_downing_marks_self_dead_when_provider_rejects_self_down() {
  let bus = Arc::new(Mutex::new(InMemoryBus::new()));
  let event_stream = EventStreamShared::default();
  let provider = ClusterProviderShared::new(Box::new(FailingSelfDownClusterProvider::new("node-a")));
  let mut node_a =
    DemoNode::new("node-a", config(), bus, event_stream).with_split_brain_resolver_downing("node-a", provider);

  let t1 = now(1);
  node_a.handle_join("node-a", "node-a", t1);
  node_a.handle_join("node-b", "node-b", t1);
  node_a.handle_join("node-c", "node-c", t1);

  let t2 = now(2);
  node_a.handle_heartbeat("node-a", t2);
  node_a.handle_heartbeat("node-b", t2);
  node_a.handle_heartbeat("node-c", t2);

  let t3 = now(3);
  node_a.handle_heartbeat("node-a", t3);
  node_a.handle_heartbeat("node-b", t3);
  node_a.handle_heartbeat("node-c", t3);

  let t4 = now(4);
  node_a.handle_heartbeat("node-a", t4);

  let t5 = now(5);
  node_a.handle_heartbeat("node-a", t5);

  let mut down_tick = 5_u64;
  assert_status_eventually("node-a", NodeStatus::Dead, 3, || {
    node_a.poll(now(down_tick));
    down_tick += 1;
    node_a.status_of("node-a")
  });
}

#[test]
fn network_partition_marks_suspect_and_recovers_after_heal() {
  let bus = Arc::new(Mutex::new(InMemoryBus::new()));
  let event_stream = EventStreamShared::default();
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
  let event_stream = EventStreamShared::default();
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
