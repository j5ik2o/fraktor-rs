use alloc::{boxed::Box, string::String, vec, vec::Vec};
use core::time::Duration;

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system;
use fraktor_actor_core_kernel_rs::{
  actor::messaging::AnyMessage,
  event::stream::{
    EventStreamEvent, EventStreamShared, EventStreamSubscriber, EventStreamSubscriberShared, EventStreamSubscription,
    subscriber_handle,
  },
};
use fraktor_utils_core_rs::{
  sync::{ArcShared, SpinSyncMutex},
  time::TimerInstant,
};

use crate::{
  BlockListProvider, ClusterError, ClusterEvent, ClusterExtension, ClusterExtensionConfig, ClusterExtensionId,
  ClusterProviderError, ClusterTopology, TopologyUpdate,
  activation::{
    ActivatedKind, IdentityLookup, IdentitySetupError, LookupError, PartitionIdentityLookup, PlacementResolution,
  },
  cluster_provider::{ClusterProvider, StaticClusterProvider},
  downing_provider::NoopDowningProvider,
  grain::{GrainKey, GrainReadiness, GrainUnreadyReason},
  membership::{Gossiper, NodeStatus},
  pub_sub::{PubSubError, PubSubSubscriber, PubSubTopic, PublishAck, PublishRequest, cluster_pub_sub::ClusterPubSub},
};

fn test_subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  subscriber_handle(subscriber)
}

fn build_update(
  hash: u64,
  members: Vec<String>,
  joined: Vec<String>,
  left: Vec<String>,
  blocked: Vec<String>,
) -> TopologyUpdate {
  let topology = ClusterTopology::new(hash, joined.clone(), left.clone(), Vec::new());
  TopologyUpdate::new(
    topology,
    members,
    joined,
    left,
    Vec::new(),
    blocked,
    TimerInstant::from_ticks(hash, Duration::from_secs(1)),
  )
}

fn publish_member_status(
  event_stream: &EventStreamShared,
  node_id: &str,
  authority: &str,
  from: NodeStatus,
  to: NodeStatus,
) {
  let cluster_event = ClusterEvent::MemberStatusChanged {
    node_id: String::from(node_id),
    authority: String::from(authority),
    from,
    to,
    observed_at: TimerInstant::from_ticks(10, Duration::from_secs(1)),
  };
  let payload = AnyMessage::new(cluster_event);
  let event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
  event_stream.publish(&event);
}

struct StubProvider;
impl ClusterProvider for StubProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
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

struct StartAndEmitSelfUpProvider {
  event_stream: EventStreamShared,
  authority:    String,
  node_id:      String,
  status:       NodeStatus,
}

impl StartAndEmitSelfUpProvider {
  const fn new(event_stream: EventStreamShared, authority: String, node_id: String) -> Self {
    Self { event_stream, authority, node_id, status: NodeStatus::Up }
  }

  const fn with_status(
    event_stream: EventStreamShared,
    authority: String,
    node_id: String,
    status: NodeStatus,
  ) -> Self {
    Self { event_stream, authority, node_id, status }
  }
}

impl ClusterProvider for StartAndEmitSelfUpProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    publish_member_status(&self.event_stream, &self.node_id, &self.authority, NodeStatus::Joining, self.status);
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    publish_member_status(&self.event_stream, &self.node_id, &self.authority, NodeStatus::Joining, self.status);
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
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

struct StartAndEmitRestartSelfUpProvider {
  event_stream:      EventStreamShared,
  authority:         String,
  first_node_id:     String,
  restarted_node_id: String,
  start_count:       usize,
}

impl StartAndEmitRestartSelfUpProvider {
  fn new(event_stream: EventStreamShared, authority: String, first_node_id: String, restarted_node_id: String) -> Self {
    Self { event_stream, authority, first_node_id, restarted_node_id, start_count: 0 }
  }

  fn current_node_id(&mut self) -> String {
    let node_id = if self.start_count == 0 { self.first_node_id.clone() } else { self.restarted_node_id.clone() };
    self.start_count += 1;
    node_id
  }
}

impl ClusterProvider for StartAndEmitRestartSelfUpProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    let node_id = self.current_node_id();
    publish_member_status(&self.event_stream, &node_id, &self.authority, NodeStatus::Joining, NodeStatus::Up);
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    let node_id = self.current_node_id();
    publish_member_status(&self.event_stream, &node_id, &self.authority, NodeStatus::Joining, NodeStatus::Up);
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
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

struct RestartEmitsSameIdentityJoiningProvider {
  event_stream: EventStreamShared,
  authority:    String,
  node_id:      String,
  start_count:  usize,
}

impl RestartEmitsSameIdentityJoiningProvider {
  const fn new(event_stream: EventStreamShared, authority: String, node_id: String) -> Self {
    Self { event_stream, authority, node_id, start_count: 0 }
  }

  fn publish_start_status(&mut self) {
    if self.start_count == 0 {
      publish_member_status(&self.event_stream, &self.node_id, &self.authority, NodeStatus::Joining, NodeStatus::Up);
    } else {
      publish_member_status(
        &self.event_stream,
        &self.node_id,
        &self.authority,
        NodeStatus::Removed,
        NodeStatus::Joining,
      );
    }
    self.start_count += 1;
  }
}

impl ClusterProvider for RestartEmitsSameIdentityJoiningProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    self.publish_start_status();
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    self.publish_start_status();
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
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

struct RestartEmitsRetiredThenCurrentProvider {
  event_stream:    EventStreamShared,
  authority:       String,
  retired_node_id: String,
  current_node_id: String,
  started_member:  bool,
}

impl RestartEmitsRetiredThenCurrentProvider {
  const fn new(
    event_stream: EventStreamShared,
    authority: String,
    retired_node_id: String,
    current_node_id: String,
  ) -> Self {
    Self { event_stream, authority, retired_node_id, current_node_id, started_member: false }
  }
}

impl ClusterProvider for RestartEmitsRetiredThenCurrentProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    if self.started_member {
      publish_member_status(
        &self.event_stream,
        &self.retired_node_id,
        &self.authority,
        NodeStatus::Joining,
        NodeStatus::Up,
      );
      publish_member_status(
        &self.event_stream,
        &self.current_node_id,
        &self.authority,
        NodeStatus::Joining,
        NodeStatus::Joining,
      );
    } else {
      self.started_member = true;
      publish_member_status(
        &self.event_stream,
        &self.retired_node_id,
        &self.authority,
        NodeStatus::Joining,
        NodeStatus::Up,
      );
    }
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
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

struct StartFirstOnlyEmitSelfUpProvider {
  event_stream:   EventStreamShared,
  authority:      String,
  node_id:        String,
  started_member: bool,
}

impl StartFirstOnlyEmitSelfUpProvider {
  const fn new(event_stream: EventStreamShared, authority: String, node_id: String) -> Self {
    Self { event_stream, authority, node_id, started_member: false }
  }
}

impl ClusterProvider for StartFirstOnlyEmitSelfUpProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    if !self.started_member {
      self.started_member = true;
      publish_member_status(&self.event_stream, &self.node_id, &self.authority, NodeStatus::Joining, NodeStatus::Up);
    }
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
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

struct StartOnceThenFailProvider {
  started_member: bool,
  started_client: bool,
}

impl StartOnceThenFailProvider {
  const fn new() -> Self {
    Self { started_member: false, started_client: false }
  }
}

impl ClusterProvider for StartOnceThenFailProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    if self.started_member {
      return Err(ClusterProviderError::start_member("restart failed"));
    }
    self.started_member = true;
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    if self.started_client {
      return Err(ClusterProviderError::start_client("restart failed"));
    }
    self.started_client = true;
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
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

struct StartEmitThenFailOnceProvider {
  event_stream: EventStreamShared,
  authority:    String,
  failed_node:  String,
  member_calls: usize,
  client_calls: usize,
}

impl StartEmitThenFailOnceProvider {
  const fn new(event_stream: EventStreamShared, authority: String, failed_node: String) -> Self {
    Self { event_stream, authority, failed_node, member_calls: 0, client_calls: 0 }
  }
}

impl ClusterProvider for StartEmitThenFailOnceProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    self.member_calls += 1;
    if self.member_calls == 2 {
      publish_member_status(
        &self.event_stream,
        &self.failed_node,
        &self.authority,
        NodeStatus::Joining,
        NodeStatus::Up,
      );
      return Err(ClusterProviderError::start_member("restart failed"));
    }
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    self.client_calls += 1;
    if self.client_calls == 2 {
      publish_member_status(
        &self.event_stream,
        &self.failed_node,
        &self.authority,
        NodeStatus::Joining,
        NodeStatus::Up,
      );
      return Err(ClusterProviderError::start_client("restart failed"));
    }
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
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

struct StubGossiper;
impl Gossiper for StubGossiper {
  fn start(&mut self) -> Result<(), &'static str> {
    Ok(())
  }

  fn stop(&mut self) -> Result<(), &'static str> {
    Ok(())
  }
}

struct StubPubSub;
impl ClusterPubSub for StubPubSub {
  fn start(&mut self) -> Result<(), PubSubError> {
    Ok(())
  }

  fn stop(&mut self) -> Result<(), PubSubError> {
    Ok(())
  }

  fn subscribe(&mut self, _topic: &PubSubTopic, _subscriber: PubSubSubscriber) -> Result<(), PubSubError> {
    Ok(())
  }

  fn unsubscribe(&mut self, _topic: &PubSubTopic, _subscriber: PubSubSubscriber) -> Result<(), PubSubError> {
    Ok(())
  }

  fn publish(&mut self, _request: PublishRequest) -> Result<PublishAck, PubSubError> {
    Ok(PublishAck::accepted())
  }

  fn on_topology(&mut self, _update: &TopologyUpdate) {}
}

struct StubIdentity;
impl IdentityLookup for StubIdentity {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, _key: &GrainKey, _now: u64) -> Result<PlacementResolution, LookupError> {
    Err(LookupError::NotReady)
  }
}

struct StubBlockList;
impl crate::BlockListProvider for StubBlockList {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

/// Helper to build `ClusterExtensionId` with default stub components.
fn stub_extension_id(config: ClusterExtensionConfig) -> ClusterExtensionId {
  ClusterExtensionId::new(
    config,
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  )
}

#[test]
fn registers_extension_and_starts_member() {
  let system = create_noop_actor_system();

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));

  let ext_shared = system.extended().register_extension(&ext_id);
  let result = ext_shared.start_member();
  assert!(result.is_ok());
}

/// Helper to build `ClusterExtensionId` with a real partition identity lookup.
fn partition_extension_id(config: ClusterExtensionConfig) -> ClusterExtensionId {
  ClusterExtensionId::new(
    config,
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  )
}

#[test]
fn grain_readiness_snapshot_transitions_from_not_ready_to_ready() {
  let system = create_noop_actor_system();
  let ext_id = partition_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);

  let expected = vec![String::from("grain-kind")];

  // 起動前: 自ノード不在・placement 未起動・kind 未登録 → NotReady（理由が観測可能）
  match ext_shared.grain_readiness_snapshot().readiness(&expected) {
    | GrainReadiness::NotReady { reasons } => {
      assert!(reasons.contains(&GrainUnreadyReason::SelfNodeNotUp { status: None }), "自ノード不在の理由が必要");
      assert!(
        reasons.iter().any(|reason| matches!(reason, GrainUnreadyReason::PlacementNotReady { .. })),
        "placement 未起動の理由が必要"
      );
      assert!(
        reasons.contains(&GrainUnreadyReason::KindNotRegistered { kind: String::from("grain-kind") }),
        "kind 未登録の理由が必要"
      );
    },
    | GrainReadiness::Ready => panic!("起動前は NotReady であるべき"),
  }

  // member 起動 + kind 登録で 3 条件を満たす
  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);
}

#[test]
fn grain_readiness_snapshot_uses_observed_self_status() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartAndEmitSelfUpProvider::with_status(
      event_stream,
      authority,
      String::from("node-self"),
      NodeStatus::Joining,
    )),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }],
  });
}

#[test]
fn grain_readiness_snapshot_clears_observed_self_status_after_shutdown() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartAndEmitSelfUpProvider::new(event_stream, authority, String::from("node-self"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);

  ext_shared.shutdown(true).expect("shutdown");

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: None }],
  });
}

#[test]
fn grain_readiness_snapshot_ignores_late_observed_self_status_after_shutdown() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartAndEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-self"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);

  ext_shared.shutdown(true).expect("shutdown");
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: None }],
  });
}

#[test]
fn grain_readiness_snapshot_ignores_observed_self_status_when_self_leaves_topology() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartAndEmitSelfUpProvider::new(event_stream, authority.clone(), String::from("node-self"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);

  ext_shared.on_topology(&build_update(20, Vec::new(), Vec::new(), vec![authority], Vec::new()));

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: None }],
  });
}

#[test]
fn grain_readiness_snapshot_accepts_observed_status_when_self_rejoins_topology() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);
  ext_shared.on_topology(&build_update(2, Vec::new(), Vec::new(), vec![String::from("fraktor://demo")], Vec::new()));
  ext_shared.on_topology(&build_update(
    3,
    vec![String::from("fraktor://demo")],
    vec![String::from("fraktor://demo")],
    Vec::new(),
    Vec::new(),
  ));

  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Removed, NodeStatus::Joining);

  match ext_shared.grain_readiness_snapshot().readiness(&expected) {
    | GrainReadiness::NotReady { reasons } => {
      assert!(
        reasons.contains(&GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }),
        "rejoined self status should remain observed as Joining"
      );
    },
    | GrainReadiness::Ready => panic!("rejoined self should not be ready while Joining"),
  }
}

#[test]
fn grain_readiness_snapshot_accepts_up_after_removed_rejoin_from_topology_absent() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address(&authority));
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);
  ext_shared.on_topology(&build_update(2, Vec::new(), Vec::new(), vec![authority.clone()], Vec::new()));
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Exiting, NodeStatus::Removed);
  ext_shared.on_topology(&build_update(3, vec![authority.clone()], vec![authority.clone()], Vec::new(), Vec::new()));
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Removed, NodeStatus::Joining);

  match ext_shared.grain_readiness_snapshot().readiness(&expected) {
    | GrainReadiness::NotReady { reasons } => {
      assert!(reasons.contains(&GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }));
    },
    | GrainReadiness::Ready => panic!("rejoined self should not be ready while Joining"),
  }

  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);

  if let GrainReadiness::NotReady { reasons } = ext_shared.grain_readiness_snapshot().readiness(&expected) {
    assert!(
      !reasons.iter().any(|reason| matches!(reason, GrainUnreadyReason::SelfNodeNotUp { .. })),
      "self Up should clear the self-node readiness blocker"
    );
  }
}

#[test]
fn grain_readiness_snapshot_keeps_removed_after_late_same_identity_up() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address(&authority));
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Exiting, NodeStatus::Removed);
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);

  match ext_shared.grain_readiness_snapshot().readiness(&expected) {
    | GrainReadiness::NotReady { reasons } => {
      assert!(reasons.contains(&GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Removed) }));
    },
    | GrainReadiness::Ready => panic!("late Up after Removed should not make readiness Ready"),
  }
}

#[test]
fn grain_readiness_snapshot_does_not_replace_removed_with_different_identity_up() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address(&authority));
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Exiting, NodeStatus::Removed);
  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Joining, NodeStatus::Up);

  match ext_shared.grain_readiness_snapshot().readiness(&expected) {
    | GrainReadiness::NotReady { reasons } => {
      assert!(reasons.contains(&GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Removed) }));
    },
    | GrainReadiness::Ready => panic!("different identity late Up after Removed should not make readiness Ready"),
  }
}

#[test]
fn grain_readiness_snapshot_ignores_old_up_before_fresh_rejoin_status() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);
  ext_shared.on_topology(&build_update(2, Vec::new(), Vec::new(), vec![String::from("fraktor://demo")], Vec::new()));
  ext_shared.on_topology(&build_update(
    3,
    vec![String::from("fraktor://demo")],
    vec![String::from("fraktor://demo")],
    Vec::new(),
    Vec::new(),
  ));

  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);
  publish_member_status(&event_stream, "node-new", "fraktor://demo", NodeStatus::Joining, NodeStatus::Joining);

  match ext_shared.grain_readiness_snapshot().readiness(&expected) {
    | GrainReadiness::NotReady { reasons } => {
      assert!(
        reasons.contains(&GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }),
        "fresh rejoin status should win over old Up"
      );
    },
    | GrainReadiness::Ready => panic!("fresh rejoin should not be ready while Joining"),
  }
}

#[test]
fn grain_readiness_snapshot_drops_stale_observed_status_when_self_rejoins_topology() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartAndEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-self"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Up, NodeStatus::Joining);
  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }],
  });

  ext_shared.on_topology(&build_update(20, Vec::new(), Vec::new(), vec![authority.clone()], Vec::new()));
  ext_shared.on_topology(&build_update(21, vec![authority], Vec::new(), Vec::new(), Vec::new()));

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);
}

#[test]
fn grain_readiness_snapshot_keeps_observed_status_on_deduped_topology() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartAndEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-self"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  ext_shared.on_topology(&build_update(20, vec![authority.clone()], Vec::new(), Vec::new(), Vec::new()));
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Up, NodeStatus::Joining);
  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }],
  });

  ext_shared.on_topology(&build_update(20, Vec::new(), Vec::new(), vec![authority], Vec::new()));

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }],
  });
}

#[test]
fn grain_readiness_snapshot_ignores_late_removed_from_previous_lifecycle_after_restart() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartAndEmitRestartSelfUpProvider::new(
      event_stream.clone(),
      authority.clone(),
      String::from("node-old"),
      String::from("node-new"),
    )),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  ext_shared.shutdown(true).expect("shutdown");
  ext_shared.start_member().expect("restart member");
  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);

  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Exiting, NodeStatus::Removed);

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);
}

#[test]
fn grain_readiness_snapshot_ignores_late_removed_before_restart_identity_is_observed() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartFirstOnlyEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-old"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  ext_shared.shutdown(true).expect("shutdown");
  ext_shared.start_member().expect("restart member");
  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);

  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Exiting, NodeStatus::Removed);

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);
}

#[test]
fn grain_readiness_snapshot_ignores_late_non_removed_then_removed_before_restart_identity_is_observed() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartFirstOnlyEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-old"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  ext_shared.shutdown(true).expect("shutdown");
  ext_shared.start_member().expect("restart member");
  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);

  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Joining, NodeStatus::Up);
  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Exiting, NodeStatus::Removed);

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);
}

#[test]
fn grain_readiness_snapshot_ignores_late_non_removed_from_older_retired_lifecycle_before_restart_identity_is_observed()
{
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartFirstOnlyEmitSelfUpProvider::new(
      event_stream.clone(),
      authority.clone(),
      String::from("node-first"),
    )),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  ext_shared.shutdown(true).expect("shutdown first lifecycle");
  ext_shared.start_member().expect("start second lifecycle");
  publish_member_status(&event_stream, "node-second", &authority, NodeStatus::Joining, NodeStatus::Up);
  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);
  ext_shared.shutdown(true).expect("shutdown second lifecycle");
  ext_shared.start_member().expect("start third lifecycle");

  publish_member_status(&event_stream, "node-first", &authority, NodeStatus::Joining, NodeStatus::Up);
  publish_member_status(&event_stream, "node-third", &authority, NodeStatus::Joining, NodeStatus::Joining);

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }],
  });
}

#[test]
fn grain_readiness_snapshot_ignores_old_removed_to_joining_from_older_retired_lifecycle() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartFirstOnlyEmitSelfUpProvider::new(
      event_stream.clone(),
      authority.clone(),
      String::from("node-first"),
    )),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  ext_shared.shutdown(true).expect("shutdown first lifecycle");
  ext_shared.start_member().expect("start second lifecycle");
  publish_member_status(&event_stream, "node-second", &authority, NodeStatus::Joining, NodeStatus::Up);
  ext_shared.shutdown(true).expect("shutdown second lifecycle");
  ext_shared.start_member().expect("start third lifecycle");

  publish_member_status(&event_stream, "node-first", &authority, NodeStatus::Removed, NodeStatus::Joining);
  publish_member_status(&event_stream, "node-third", &authority, NodeStatus::Joining, NodeStatus::Joining);

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }],
  });
}

#[test]
fn grain_readiness_snapshot_ignores_retired_identity_arriving_during_restart_start_window() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(RestartEmitsRetiredThenCurrentProvider::new(
      event_stream,
      authority.clone(),
      String::from("node-old"),
      String::from("node-new"),
    )),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  ext_shared.shutdown(true).expect("shutdown");
  ext_shared.start_member().expect("restart member");

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }],
  });
}

#[test]
fn grain_readiness_snapshot_accepts_same_node_id_rejoin_after_restart_start_window() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartFirstOnlyEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-self"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  ext_shared.shutdown(true).expect("shutdown");
  ext_shared.start_member().expect("restart member");

  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Removed, NodeStatus::Joining);

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }],
  });

  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-self"), String::from("fraktor://demo"))]);
}

#[test]
fn grain_readiness_snapshot_accepts_same_node_id_rejoin_during_restart_start_window() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(RestartEmitsSameIdentityJoiningProvider::new(event_stream, authority.clone(), String::from("node-self"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  ext_shared.shutdown(true).expect("shutdown");
  ext_shared.start_member().expect("restart member");

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }],
  });
}

#[test]
fn grain_readiness_snapshot_accepts_same_node_id_up_during_restart_start_window() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartAndEmitRestartSelfUpProvider::new(
      event_stream,
      authority.clone(),
      String::from("node-self"),
      String::from("node-self"),
    )),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  ext_shared.shutdown(true).expect("shutdown");
  ext_shared.start_member().expect("restart member");

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-self"), String::from("fraktor://demo"))]);
}

#[test]
fn grain_readiness_snapshot_accepts_removed_from_new_lifecycle_before_restart_identity_is_observed() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartFirstOnlyEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-old"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  ext_shared.shutdown(true).expect("shutdown");
  ext_shared.start_member().expect("restart member");
  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::Ready);

  publish_member_status(&event_stream, "node-new", &authority, NodeStatus::Exiting, NodeStatus::Removed);

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Removed) }],
  });
}

#[test]
fn grain_readiness_snapshot_ignores_late_non_removed_from_previous_lifecycle_after_restart_identity_is_observed() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartFirstOnlyEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-old"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(PartitionIdentityLookup::with_defaults()),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  let expected = vec![String::from("grain-kind")];

  ext_shared.start_member().expect("start member");
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).expect("setup kinds");
  ext_shared.shutdown(true).expect("shutdown");
  ext_shared.start_member().expect("restart member");
  publish_member_status(&event_stream, "node-new", &authority, NodeStatus::Joining, NodeStatus::Joining);
  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }],
  });

  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Joining, NodeStatus::Up);

  assert_eq!(ext_shared.grain_readiness_snapshot().readiness(&expected), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }],
  });
}

#[test]
fn register_on_member_up_invokes_callback_for_up_transition() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  publish_member_status(&event_stream, "node-ignored", "node-other", NodeStatus::Joining, NodeStatus::Up);
  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);
  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Suspect, NodeStatus::Up);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-self"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_removed_invokes_callback_for_removed_transition() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  publish_member_status(&event_stream, "node-ignored", "node-other", NodeStatus::Exiting, NodeStatus::Removed);
  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);
  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Dead, NodeStatus::Removed);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-self"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_up_invokes_callback_immediately_when_self_already_up() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);

  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Suspect, NodeStatus::Up);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-self"), String::from("fraktor://demo"))]);
}

fn run_member_up_during_start_test(start_fn: impl FnOnce(&ClusterExtension) -> Result<(), ClusterError>) {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartAndEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-self"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  start_fn(&ext_shared).expect("start");

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-self"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_up_invokes_callback_when_status_arrives_during_start_member() {
  run_member_up_during_start_test(|ext| ext.start_member());
}

#[test]
fn register_on_member_up_invokes_callback_when_status_arrives_during_start_client() {
  run_member_up_during_start_test(|ext| ext.start_client());
}

fn run_member_up_during_restart_test(start_fn: impl Fn(&ClusterExtension) -> Result<(), ClusterError>) {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartAndEmitRestartSelfUpProvider::new(
      event_stream.clone(),
      authority.clone(),
      String::from("node-old"),
      String::from("node-new"),
    )),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  start_fn(&ext_shared).expect("start");
  ext_shared.shutdown(true).expect("shutdown");
  start_fn(&ext_shared).expect("restart");

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-new"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_up_invokes_callback_when_status_arrives_during_restart_member() {
  run_member_up_during_restart_test(|ext| ext.start_member());
}

#[test]
fn register_on_member_up_invokes_callback_when_status_arrives_during_restart_client() {
  run_member_up_during_restart_test(|ext| ext.start_client());
}

#[test]
fn register_on_member_removed_invokes_callback_immediately_when_self_already_removed() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);

  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Dead, NodeStatus::Removed);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-self"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_removed_invokes_callback_immediately_after_shutdown() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.start_member().expect("start member");
  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);
  ext_shared.shutdown(true).expect("shutdown");

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-self"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_removed_after_shutdown_ignores_late_non_removed_identity() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.start_member().expect("start member");
  publish_member_status(&event_stream, "node-removed", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);
  ext_shared.shutdown(true).expect("shutdown");
  publish_member_status(&event_stream, "node-late", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-removed"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_removed_after_shutdown_ignores_late_removed_from_previous_lifecycle() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartFirstOnlyEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-old"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  ext_shared.start_member().expect("start member");
  ext_shared.shutdown(true).expect("shutdown");
  ext_shared.start_member().expect("restart member");
  publish_member_status(&event_stream, "node-new", &authority, NodeStatus::Joining, NodeStatus::Up);
  ext_shared.shutdown(true).expect("shutdown again");
  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Exiting, NodeStatus::Removed);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-new"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_removed_after_shutdown_ignores_retired_removed_when_current_identity_unknown() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartFirstOnlyEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-old"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  ext_shared.start_member().expect("start member");
  ext_shared.shutdown(true).expect("shutdown");
  ext_shared.start_member().expect("restart member");
  ext_shared.shutdown(true).expect("shutdown again");
  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Exiting, NodeStatus::Removed);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("fraktor://demo"), String::from("fraktor://demo"))]);
}

fn run_failed_restart_preserves_removed_identity_test(
  start_fn: impl Fn(&ClusterExtension) -> Result<(), ClusterError>,
) {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    Box::new(StartOnceThenFailProvider::new()),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  start_fn(&ext_shared).expect("start");
  publish_member_status(&event_stream, "node-removed", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);
  ext_shared.shutdown(true).expect("shutdown");
  assert!(start_fn(&ext_shared).is_err());

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-removed"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_removed_after_failed_member_restart_preserves_removed_identity() {
  run_failed_restart_preserves_removed_identity_test(|ext| ext.start_member());
}

#[test]
fn register_on_member_removed_after_failed_client_restart_preserves_removed_identity() {
  run_failed_restart_preserves_removed_identity_test(|ext| ext.start_client());
}

#[test]
fn register_on_member_removed_after_shutdown_falls_back_to_authority_when_node_id_is_unknown() {
  let system = create_noop_actor_system();

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.start_member().expect("start member");
  ext_shared.shutdown(true).expect("shutdown");

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("fraktor://demo"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_up_does_not_fire_for_buffered_old_up_events() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);

  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);
  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  assert!(calls.lock().is_empty());

  publish_member_status(&event_stream, "node-new", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-new"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_up_does_not_fire_for_retired_up_after_restart() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartFirstOnlyEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-old"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  ext_shared.start_member().expect("start member");
  ext_shared.shutdown(true).expect("shutdown");
  ext_shared.start_member().expect("restart member");

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Joining, NodeStatus::Up);
  assert!(calls.lock().is_empty());

  publish_member_status(&event_stream, "node-new", &authority, NodeStatus::Joining, NodeStatus::Up);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-new"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_up_does_not_fire_after_shutdown() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address(&authority));
  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.start_member().expect("start member");

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  ext_shared.shutdown(true).expect("shutdown");
  publish_member_status(&event_stream, "node-late", &authority, NodeStatus::Joining, NodeStatus::Up);

  assert!(calls.lock().is_empty());
}

#[test]
fn register_on_member_up_does_not_fire_after_topology_absent_removed() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address(&authority));
  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.start_member().expect("start member");
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);
  ext_shared.on_topology(&build_update(2, Vec::new(), Vec::new(), vec![authority.clone()], Vec::new()));
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Exiting, NodeStatus::Removed);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);

  assert!(calls.lock().is_empty());
}

#[test]
fn register_on_member_up_does_not_fire_for_late_up_after_removed() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address(&authority));
  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.start_member().expect("start member");
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Exiting, NodeStatus::Removed);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);
  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Joining, NodeStatus::Up);

  assert!(calls.lock().is_empty());
}

#[test]
fn register_on_member_up_does_not_fire_for_identity_mismatched_up() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address(&authority));
  let ext_shared = system.extended().register_extension(&ext_id);

  publish_member_status(&event_stream, "node-new", &authority, NodeStatus::Joining, NodeStatus::Joining);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Joining, NodeStatus::Up);
  assert!(calls.lock().is_empty());

  publish_member_status(&event_stream, "node-new", &authority, NodeStatus::Joining, NodeStatus::Up);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-new"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_removed_fires_after_self_leaves_topology() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address(&authority));
  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.start_member().expect("start member");
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  ext_shared.on_topology(&build_update(2, Vec::new(), Vec::new(), vec![authority.clone()], Vec::new()));
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Exiting, NodeStatus::Removed);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-self"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_removed_invokes_immediately_after_removed_then_topology_absent() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address(&authority));
  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.start_member().expect("start member");
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Joining, NodeStatus::Up);
  publish_member_status(&event_stream, "node-self", &authority, NodeStatus::Exiting, NodeStatus::Removed);
  ext_shared.on_topology(&build_update(2, Vec::new(), Vec::new(), vec![authority], Vec::new()));

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-self"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_removed_does_not_fire_for_identity_mismatched_removed() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address(&authority));
  let ext_shared = system.extended().register_extension(&ext_id);

  publish_member_status(&event_stream, "node-new", &authority, NodeStatus::Joining, NodeStatus::Up);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Exiting, NodeStatus::Removed);
  assert!(calls.lock().is_empty());

  publish_member_status(&event_stream, "node-new", &authority, NodeStatus::Exiting, NodeStatus::Removed);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-new"), String::from("fraktor://demo"))]);
}

fn run_failed_restart_retires_failed_start_identity_test(
  start_fn: impl Fn(&ClusterExtension) -> Result<(), ClusterError>,
) {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartEmitThenFailOnceProvider::new(event_stream.clone(), authority.clone(), String::from("node-failed"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  start_fn(&ext_shared).expect("start");
  publish_member_status(&event_stream, "node-old", &authority, NodeStatus::Joining, NodeStatus::Up);
  ext_shared.shutdown(true).expect("shutdown");
  assert!(start_fn(&ext_shared).is_err());
  start_fn(&ext_shared).expect("restart");

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  publish_member_status(&event_stream, "node-failed", &authority, NodeStatus::Joining, NodeStatus::Up);
  assert!(calls.lock().is_empty());

  publish_member_status(&event_stream, "node-new", &authority, NodeStatus::Joining, NodeStatus::Up);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-new"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_up_ignores_failed_member_start_identity_after_retry() {
  run_failed_restart_retires_failed_start_identity_test(|ext| ext.start_member());
}

#[test]
fn register_on_member_up_ignores_failed_client_start_identity_after_retry() {
  run_failed_restart_retires_failed_start_identity_test(|ext| ext.start_client());
}

#[test]
fn register_on_member_removed_does_not_fire_for_buffered_old_removed_events() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);

  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);
  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  assert!(calls.lock().is_empty());

  publish_member_status(&event_stream, "node-new", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-new"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_up_does_not_fire_for_events_buffered_before_extension_install() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  assert!(calls.lock().is_empty());

  publish_member_status(&event_stream, "node-new", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-new"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_removed_does_not_fire_for_events_buffered_before_extension_install() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);

  let ext_id = stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"));
  let ext_shared = system.extended().register_extension(&ext_id);

  let calls = ArcShared::new(SpinSyncMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  assert!(calls.lock().is_empty());

  publish_member_status(&event_stream, "node-new", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-new"), String::from("fraktor://demo"))]);
}

#[test]
fn subscribes_to_event_stream_and_applies_topology_on_topology_updated() {
  // 1. システムとエクステンションをセットアップ
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = stub_extension_id(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo").with_metrics_enabled(true),
  );

  // 2. エクステンションを登録
  let ext_shared = system.extended().register_extension(&ext_id);

  // 3. エクステンションを開始（この時点で EventStream を購読するべき）
  ext_shared.start_member().unwrap();

  // 4. EventStream に TopologyUpdated イベントを publish
  let update = build_update(
    12345,
    vec![String::from("fraktor://demo"), String::from("node-b")],
    vec![String::from("node-b")],
    vec![],
    vec![],
  );
  let cluster_event = ClusterEvent::TopologyUpdated { update };
  let payload = AnyMessage::new(cluster_event);
  let event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
  event_stream.publish(&event);

  // 5. ClusterExtension が自動的に ClusterCore::on_topology を呼んだことを確認
  // metrics が更新されていればトポロジが適用されたことになる
  let metrics = ext_shared.metrics().unwrap();
  // start_member で members=1、topology で +1 joined なので members=2 を期待
  assert_eq!(metrics.members(), 2);
}

#[test]
fn ignores_topology_with_same_hash_via_event_stream() {
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  let ext_id = stub_extension_id(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo").with_metrics_enabled(true),
  );

  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.start_member().unwrap();

  // 同じハッシュのトポロジを2回 publish
  for _ in 0..2 {
    let update = build_update(
      99999,
      vec![String::from("fraktor://demo"), String::from("node-x")],
      vec![String::from("node-x")],
      vec![],
      vec![],
    );
    let cluster_event = ClusterEvent::TopologyUpdated { update };
    let payload = AnyMessage::new(cluster_event);
    let event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    event_stream.publish(&event);
  }

  // 重複ハッシュは抑止されるので、members は 1(initial) + 1(first topology) = 2 のまま
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 2);
}

// ====================================================================
// Phase1 統合テスト: 静的トポロジ publish → EventStream → ClusterCore
// 要件1.1, 1.2, 1.4, 3.3, 5.1, 5.3 をカバー
// ====================================================================

/// BlockListProvider を実装したスタブ（blocked メンバーを返す）
struct RecordingBlockList {
  blocked: Vec<String>,
}

impl RecordingBlockList {
  fn new(blocked: Vec<String>) -> Self {
    Self { blocked }
  }
}

impl BlockListProvider for RecordingBlockList {
  fn blocked_members(&self) -> Vec<String> {
    self.blocked.clone()
  }
}

/// ClusterEvent を記録する EventStream subscriber
#[derive(Clone)]
struct RecordingClusterEvents {
  events: ArcShared<SpinSyncMutex<Vec<ClusterEvent>>>,
}

impl RecordingClusterEvents {
  fn new() -> Self {
    Self { events: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<ClusterEvent> {
    self.events.lock().clone()
  }

  fn topology_updated_events(&self) -> Vec<ClusterEvent> {
    self.events().into_iter().filter(|e| matches!(e, ClusterEvent::TopologyUpdated { .. })).collect()
  }
}

impl EventStreamSubscriber for RecordingClusterEvents {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == "cluster"
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
    {
      self.events.lock().push(cluster_event.clone());
    }
  }
}

fn subscribe_recorder(event_stream: &EventStreamShared) -> (RecordingClusterEvents, EventStreamSubscription) {
  let recorder = RecordingClusterEvents::new();
  let subscriber = test_subscriber_handle(recorder.clone());
  let subscription = event_stream.subscribe(&subscriber);
  (recorder, subscription)
}

/// Phase1 統合テスト: StaticClusterProvider の静的トポロジが EventStream に publish され、
/// ClusterExtension が自動的に購読して ClusterCore に適用することを検証
#[test]
fn phase1_integration_static_topology_publishes_to_event_stream_and_applies_to_core() {
  // 1. システムをセットアップ
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録してイベントを記録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. StaticClusterProvider を静的トポロジで構成
  let block_list: ArcShared<dyn BlockListProvider> =
    ArcShared::new(RecordingBlockList::new(vec![String::from("blocked-node-a")]));
  let static_topology =
    ClusterTopology::new(1000, vec![String::from("node-b"), String::from("node-c")], vec![], Vec::new());
  let provider = StaticClusterProvider::new(event_stream.clone(), block_list.clone(), "node-a")
    .with_static_topology(static_topology);

  // 4. ClusterExtension をセットアップ（metrics 有効）
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(provider),
    block_list,
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).unwrap();

  // 5. クラスタを開始（start_member で provider が静的トポロジを publish）
  ext_shared.start_member().unwrap();

  // 6. EventStream に TopologyUpdated が publish されたことを確認
  let topology_events = recorder.topology_updated_events();
  assert!(!topology_events.is_empty(), "TopologyUpdated should be published to EventStream");

  // 7. ClusterCore の metrics が更新されたことを確認
  // start_member で members=1、静的トポロジで joined=2 なので members=3 を期待
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 3, "Members should include initial + joined nodes");

  // 8. blocked メンバーが反映されていることを確認
  let blocked = ext_shared.blocked_members();
  assert!(blocked.contains(&String::from("blocked-node-a")), "Blocked members should be reflected");
}

// Note: PIDキャッシュの無効化テストは cluster_core_test.rs の
// topology_event_includes_blocked_and_updates_metrics と
// multi_node_topology_flow_updates_metrics_and_pid_cache で既にカバーされている

/// Phase1 統合テスト: blocked メンバーが TopologyUpdated イベントに含まれることを検証
#[test]
fn phase1_integration_topology_updated_includes_blocked_members() {
  // 1. システムをセットアップ
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. BlockList に複数のブロックされたノードを設定
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(RecordingBlockList::new(vec![
    String::from("blocked-1"),
    String::from("blocked-2"),
    String::from("blocked-3"),
  ]));

  // 4. StaticClusterProvider を設定
  let static_topology = ClusterTopology::new(3000, vec![String::from("node-b")], vec![], Vec::new());
  let provider = StaticClusterProvider::new(event_stream.clone(), block_list.clone(), "node-a")
    .with_static_topology(static_topology);

  // 5. ClusterExtension をセットアップ
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(provider),
    block_list,
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  // 6. クラスタを開始
  ext_shared.start_member().unwrap();

  // 7. TopologyUpdated イベントに blocked が含まれていることを確認
  let topology_events = recorder.topology_updated_events();
  assert!(!topology_events.is_empty());

  if let ClusterEvent::TopologyUpdated { update } = &topology_events[0] {
    assert!(update.blocked.contains(&String::from("blocked-1")));
    assert!(update.blocked.contains(&String::from("blocked-2")));
    assert!(update.blocked.contains(&String::from("blocked-3")));
  } else {
    panic!("Expected TopologyUpdated event");
  }

  // 8. ClusterExtension.blocked_members() からも取得できることを確認
  let ext_blocked = ext_shared.blocked_members();
  assert!(ext_blocked.contains(&String::from("blocked-1")));
  assert!(ext_blocked.contains(&String::from("blocked-2")));
  assert!(ext_blocked.contains(&String::from("blocked-3")));
}

/// Phase1 統合テスト: ハッシュが同一のトポロジは EventStream に重複 publish されないことを検証
#[test]
fn phase1_integration_duplicate_hash_topology_is_suppressed() {
  // 1. システムをセットアップ
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. ClusterExtension をセットアップ
  let ext_id =
    stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true));
  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.start_member().unwrap();

  // 4. 同じハッシュのトポロジを複数回適用
  let update = build_update(
    5000,
    vec![String::from("node-a"), String::from("node-x")],
    vec![String::from("node-x")],
    vec![],
    vec![],
  );
  ext_shared.on_topology(&update);
  ext_shared.on_topology(&update); // 重複
  ext_shared.on_topology(&update); // 重複

  // 5. TopologyUpdated は1回だけ publish されるべき
  let topology_events = recorder.topology_updated_events();
  assert_eq!(topology_events.len(), 1, "Duplicate hash topology should be suppressed");
}

/// Phase1 統合テスト: metrics 更新が正しく行われることを検証（virtual_actors 含む）
#[test]
fn phase1_integration_metrics_include_members_and_virtual_actors() {
  // 1. システムをセットアップ
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  // 2. ClusterExtension をセットアップ
  let ext_id =
    stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true));
  let ext_shared = system.extended().register_extension(&ext_id);

  // 3. Kind を登録（virtual_actors が増加する）
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("worker-kind"), ActivatedKind::new("analytics-kind")]).unwrap();

  // 4. クラスタを開始
  ext_shared.start_member().unwrap();

  // 5. 初期メトリクスを確認（members=1, virtual_actors=3: worker + analytics + topic）
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 1);
  assert_eq!(metrics.virtual_actors(), 3);

  // 6. トポロジを更新（2ノード join）
  let update = build_update(
    6000,
    vec![String::from("node-a"), String::from("node-b"), String::from("node-c")],
    vec![String::from("node-b"), String::from("node-c")],
    vec![],
    vec![],
  );
  let cluster_event = ClusterEvent::TopologyUpdated { update };
  let payload = AnyMessage::new(cluster_event);
  let event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
  event_stream.publish(&event);

  // 7. メトリクスが更新されたことを確認
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 3, "Members should be 1 + 2 joined");
  assert_eq!(metrics.virtual_actors(), 3, "Virtual actors should remain unchanged");
}

// ====================================================================
// Phase2 統合テスト（タスク 4.4）
// join/leave/BlockList 反映・metrics 更新・EventStream TopologyUpdated 出力を確認
// ====================================================================

/// Phase2 統合テスト: join/leave イベントが EventStream に TopologyUpdated として出力される
#[test]
fn phase2_integration_join_leave_events_produce_topology_updated() {
  // 1. システムをセットアップ
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. ClusterExtension をセットアップ
  let ext_id =
    stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true));
  let ext_shared = system.extended().register_extension(&ext_id);

  // 4. クラスタを開始
  ext_shared.start_member().unwrap();

  // 5. ノード join のトポロジ更新
  let join_update = build_update(
    100,
    vec![String::from("node-a"), String::from("node-b"), String::from("node-c")],
    vec![String::from("node-b"), String::from("node-c")],
    vec![],
    vec![],
  );
  ext_shared.on_topology(&join_update);

  // 6. ノード leave のトポロジ更新
  let leave_update = build_update(
    200,
    vec![String::from("node-a"), String::from("node-b")],
    vec![],
    vec![String::from("node-c")],
    vec![],
  );
  ext_shared.on_topology(&leave_update);

  // 7. TopologyUpdated イベントが発火されたことを確認
  let topology_events = recorder.topology_updated_events();
  assert!(topology_events.len() >= 2, "At least 2 TopologyUpdated events should be fired");

  // 8. join イベントを確認
  assert!(
    topology_events.iter().any(|e| matches!(
      e,
      ClusterEvent::TopologyUpdated { update }
      if update.joined.contains(&String::from("node-b"))
    )),
    "TopologyUpdated should contain node-b in joined"
  );

  // 9. leave イベントを確認
  assert!(
    topology_events.iter().any(|e| matches!(
      e,
      ClusterEvent::TopologyUpdated { update }
      if update.left.contains(&String::from("node-c"))
    )),
    "TopologyUpdated should contain node-c in left"
  );
}

/// Phase2 統合テスト: BlockList が TopologyUpdated イベントに反映される
#[test]
fn phase2_integration_blocklist_reflected_in_topology_events() {
  // 1. システムをセットアップ
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. BlockList に複数のノードを設定
  let block_list: ArcShared<dyn BlockListProvider> =
    ArcShared::new(RecordingBlockList::new(vec![String::from("blocked-node-1"), String::from("blocked-node-2")]));

  // 4. ClusterExtension をセットアップ
  let ext_id = ClusterExtensionId::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(StubProvider),
    block_list,
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  // 5. クラスタを開始
  ext_shared.start_member().unwrap();

  // 6. トポロジ更新を行う
  let update = build_update(
    300,
    vec![String::from("node-a"), String::from("node-b")],
    vec![String::from("node-b")],
    vec![],
    vec![String::from("blocked-node-1"), String::from("blocked-node-2")],
  );
  ext_shared.on_topology(&update);

  // 7. TopologyUpdated イベントに blocked が含まれていることを確認
  let topology_events = recorder.topology_updated_events();
  assert!(!topology_events.is_empty(), "TopologyUpdated should be fired");

  // 8. blocked メンバーが含まれていることを確認
  let has_blocked = topology_events.iter().any(|e| {
    if let ClusterEvent::TopologyUpdated { update } = e {
      update.blocked.contains(&String::from("blocked-node-1"))
        && update.blocked.contains(&String::from("blocked-node-2"))
    } else {
      false
    }
  });
  assert!(has_blocked, "TopologyUpdated should contain blocked members");

  // 9. ClusterExtension からも blocked を取得できることを確認
  let ext_blocked = ext_shared.blocked_members();
  assert!(ext_blocked.contains(&String::from("blocked-node-1")));
  assert!(ext_blocked.contains(&String::from("blocked-node-2")));
}

/// Phase2 統合テスト: metrics が正しく更新される
#[test]
fn phase2_integration_metrics_updated_correctly_with_dynamic_topology() {
  // 1. システムをセットアップ
  let system = create_noop_actor_system();

  // 2. ClusterExtension をセットアップ
  let ext_id =
    stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true));
  let ext_shared = system.extended().register_extension(&ext_id);

  // 3. Kind を登録して起動
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("worker-kind")]).unwrap();
  ext_shared.start_member().unwrap();

  // 4. 初期メトリクス確認
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 1, "Initial members should be 1");
  assert_eq!(metrics.virtual_actors(), 2, "worker + topic = 2 virtual actors");

  // 5. 3ノード join
  let update1 = build_update(
    400,
    vec![String::from("node-a"), String::from("node-b"), String::from("node-c"), String::from("node-d")],
    vec![String::from("node-b"), String::from("node-c"), String::from("node-d")],
    vec![],
    vec![],
  );
  ext_shared.on_topology(&update1);

  // 6. メトリクスが更新されたことを確認
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 4, "Members should be 1 + 3 joined = 4");

  // 7. 2ノード leave
  let update2 = build_update(
    500,
    vec![String::from("node-a"), String::from("node-c")],
    vec![],
    vec![String::from("node-b"), String::from("node-d")],
    vec![],
  );
  ext_shared.on_topology(&update2);

  // 8. メトリクスが更新されたことを確認
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 2, "Members should be 4 - 2 left = 2");

  // 9. virtual_actors は変化しないことを確認
  assert_eq!(metrics.virtual_actors(), 2, "Virtual actors should remain 2");
}

/// Phase2 統合テスト: shutdown 後のメトリクスリセット
#[test]
fn phase2_integration_shutdown_resets_metrics_and_emits_event() {
  // 1. システムをセットアップ
  let system = create_noop_actor_system();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. ClusterExtension をセットアップ
  let ext_id =
    stub_extension_id(ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true));
  let ext_shared = system.extended().register_extension(&ext_id);

  // 4. Kind を登録して起動
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("worker-kind")]).unwrap();
  ext_shared.start_member().unwrap();

  // 5. トポロジ更新を行う
  let update = build_update(
    600,
    vec![String::from("node-a"), String::from("node-b")],
    vec![String::from("node-b")],
    vec![],
    vec![],
  );
  ext_shared.on_topology(&update);

  // 6. shutdown を呼ぶ
  ext_shared.shutdown(true).unwrap();

  // 7. Shutdown イベントが発火されたことを確認
  let events = recorder.events();
  assert!(
    events.iter().any(|e| matches!(
      e,
      ClusterEvent::Shutdown { address, mode }
      if address == "node-a" && *mode == crate::StartupMode::Member
    )),
    "Shutdown event should be fired"
  );

  // 8. virtual_actor_count がリセットされていることを確認
  assert_eq!(ext_shared.virtual_actor_count(), 0, "virtual_actor_count should be reset after shutdown");

  // 9. blocked_members がクリアされていることを確認
  assert!(ext_shared.blocked_members().is_empty(), "blocked_members should be cleared after shutdown");
}
