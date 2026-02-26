use alloc::{boxed::Box, string::String, vec, vec::Vec};
use core::time::Duration;

use fraktor_actor_rs::core::{
  event::stream::{
    EventStreamEvent, EventStreamSharedGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric,
    subscriber_handle,
  },
  messaging::AnyMessageGeneric,
  system::ActorSystemGeneric,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
  time::TimerInstant,
};

use crate::core::{
  ClusterError, ClusterEvent, ClusterExtensionConfig, ClusterExtensionGeneric, ClusterExtensionId,
  ClusterProviderError, ClusterTopology, TopologyUpdate,
  cluster_provider::{ClusterProvider, StaticClusterProvider},
  downing_provider::NoopDowningProvider,
  grain::GrainKey,
  identity::{IdentityLookup, IdentitySetupError, LookupError},
  membership::{Gossiper, NodeStatus},
  placement::{ActivatedKind, PlacementResolution},
  pub_sub::cluster_pub_sub::ClusterPubSub,
};

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
  event_stream: &EventStreamSharedGeneric<NoStdToolbox>,
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
  let payload = AnyMessageGeneric::new(cluster_event);
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

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

struct StartAndEmitSelfUpProvider {
  event_stream: EventStreamSharedGeneric<NoStdToolbox>,
  authority:    String,
  node_id:      String,
}

impl StartAndEmitSelfUpProvider {
  const fn new(event_stream: EventStreamSharedGeneric<NoStdToolbox>, authority: String, node_id: String) -> Self {
    Self { event_stream, authority, node_id }
  }
}

impl ClusterProvider for StartAndEmitSelfUpProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    publish_member_status(&self.event_stream, &self.node_id, &self.authority, NodeStatus::Joining, NodeStatus::Up);
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    publish_member_status(&self.event_stream, &self.node_id, &self.authority, NodeStatus::Joining, NodeStatus::Up);
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
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
impl ClusterPubSub<NoStdToolbox> for StubPubSub {
  fn start(&mut self) -> Result<(), crate::core::pub_sub::PubSubError> {
    Ok(())
  }

  fn stop(&mut self) -> Result<(), crate::core::pub_sub::PubSubError> {
    Ok(())
  }

  fn subscribe(
    &mut self,
    _topic: &crate::core::pub_sub::PubSubTopic,
    _subscriber: crate::core::pub_sub::PubSubSubscriber<NoStdToolbox>,
  ) -> Result<(), crate::core::pub_sub::PubSubError> {
    Ok(())
  }

  fn unsubscribe(
    &mut self,
    _topic: &crate::core::pub_sub::PubSubTopic,
    _subscriber: crate::core::pub_sub::PubSubSubscriber<NoStdToolbox>,
  ) -> Result<(), crate::core::pub_sub::PubSubError> {
    Ok(())
  }

  fn publish(
    &mut self,
    _request: crate::core::pub_sub::PublishRequest<NoStdToolbox>,
  ) -> Result<crate::core::pub_sub::PublishAck, crate::core::pub_sub::PubSubError> {
    Ok(crate::core::pub_sub::PublishAck::accepted())
  }

  fn on_topology(&mut self, _update: &crate::core::TopologyUpdate) {}
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
impl fraktor_remote_rs::core::BlockListProvider for StubBlockList {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

#[test]
fn registers_extension_and_starts_member() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );

  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");
  let result = ext_shared.start_member();
  assert!(result.is_ok());
}

#[test]
fn register_on_member_up_invokes_callback_for_up_transition() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

  let calls = ArcShared::new(NoStdMutex::new(Vec::<(String, String)>::new()));
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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

  let calls = ArcShared::new(NoStdMutex::new(Vec::<(String, String)>::new()));
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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);

  let calls = ArcShared::new(NoStdMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_up(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Suspect, NodeStatus::Up);

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-self"), String::from("fraktor://demo"))]);
}

fn run_member_up_during_start_test(
  start_fn: impl FnOnce(&ClusterExtensionGeneric<NoStdToolbox>) -> Result<(), ClusterError>,
) {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();
  let authority = String::from("fraktor://demo");

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address(&authority),
    Box::new(StartAndEmitSelfUpProvider::new(event_stream.clone(), authority.clone(), String::from("node-self"))),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

  start_fn(&ext_shared).expect("start");

  let calls = ArcShared::new(NoStdMutex::new(Vec::<(String, String)>::new()));
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

#[test]
fn register_on_member_removed_invokes_callback_immediately_when_self_already_removed() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);

  let calls = ArcShared::new(NoStdMutex::new(Vec::<(String, String)>::new()));
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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");
  ext_shared.start_member().expect("start member");
  publish_member_status(&event_stream, "node-self", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);
  ext_shared.shutdown(true).expect("shutdown");

  let calls = ArcShared::new(NoStdMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("node-self"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_removed_after_shutdown_falls_back_to_authority_when_node_id_is_unknown() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");
  ext_shared.start_member().expect("start member");
  ext_shared.shutdown(true).expect("shutdown");

  let calls = ArcShared::new(NoStdMutex::new(Vec::<(String, String)>::new()));
  let calls_for_callback = calls.clone();
  let _subscription = ext_shared.register_on_member_removed(move |node_id, authority| {
    calls_for_callback.lock().push((String::from(node_id), String::from(authority)));
  });

  let recorded = calls.lock().clone();
  assert_eq!(recorded, vec![(String::from("fraktor://demo"), String::from("fraktor://demo"))]);
}

#[test]
fn register_on_member_up_does_not_fire_for_buffered_old_up_events() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);
  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);

  let calls = ArcShared::new(NoStdMutex::new(Vec::<(String, String)>::new()));
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
fn register_on_member_removed_does_not_fire_for_buffered_old_removed_events() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);
  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);

  let calls = ArcShared::new(NoStdMutex::new(Vec::<(String, String)>::new()));
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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Joining, NodeStatus::Up);

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

  let calls = ArcShared::new(NoStdMutex::new(Vec::<(String, String)>::new()));
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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  publish_member_status(&event_stream, "node-old", "fraktor://demo", NodeStatus::Exiting, NodeStatus::Removed);

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

  let calls = ArcShared::new(NoStdMutex::new(Vec::<(String, String)>::new()));
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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo").with_metrics_enabled(true),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );

  // 2. エクステンションを登録
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

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
  let payload = AnyMessageGeneric::new(cluster_event);
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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo").with_metrics_enabled(true),
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );

  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");
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
    let payload = AnyMessageGeneric::new(cluster_event);
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
  events: ArcShared<NoStdMutex<Vec<ClusterEvent>>>,
}

impl RecordingClusterEvents {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<ClusterEvent> {
    self.events.lock().clone()
  }

  fn topology_updated_events(&self) -> Vec<ClusterEvent> {
    self.events().into_iter().filter(|e| matches!(e, ClusterEvent::TopologyUpdated { .. })).collect()
  }
}

impl EventStreamSubscriber<NoStdToolbox> for RecordingClusterEvents {
  fn on_event(&mut self, event: &EventStreamEvent<NoStdToolbox>) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == "cluster"
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
    {
      self.events.lock().push(cluster_event.clone());
    }
  }
}

fn subscribe_recorder(
  event_stream: &EventStreamSharedGeneric<NoStdToolbox>,
) -> (RecordingClusterEvents, EventStreamSubscriptionGeneric<NoStdToolbox>) {
  let recorder = RecordingClusterEvents::new();
  let subscriber = subscriber_handle(recorder.clone());
  let subscription = event_stream.subscribe(&subscriber);
  (recorder, subscription)
}

/// Phase1 統合テスト: StaticClusterProvider の静的トポロジが EventStream に publish され、
/// ClusterExtension が自動的に購読して ClusterCore に適用することを検証
#[test]
fn phase1_integration_static_topology_publishes_to_event_stream_and_applies_to_core() {
  // 1. システムをセットアップ
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
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
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(provider),
    block_list,
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");
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

// Note: PIDキャッシュの無効化テストは cluster_core/tests.rs の
// topology_event_includes_blocked_and_updates_metrics と
// multi_node_topology_flow_updates_metrics_and_pid_cache で既にカバーされている

/// Phase1 統合テスト: blocked メンバーが TopologyUpdated イベントに含まれることを検証
#[test]
fn phase1_integration_topology_updated_includes_blocked_members() {
  // 1. システムをセットアップ
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
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
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(provider),
    block_list,
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. ClusterExtension をセットアップ
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(StubBlockList);
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(StubProvider),
    block_list,
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");
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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. ClusterExtension をセットアップ
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(StubBlockList);
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(StubProvider),
    block_list,
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

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
  let payload = AnyMessageGeneric::new(cluster_event);
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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. ClusterExtension をセットアップ
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(StubBlockList);
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(StubProvider),
    block_list,
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. BlockList に複数のノードを設定
  let block_list: ArcShared<dyn BlockListProvider> =
    ArcShared::new(RecordingBlockList::new(vec![String::from("blocked-node-1"), String::from("blocked-node-2")]));

  // 4. ClusterExtension をセットアップ
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(StubProvider),
    block_list,
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();

  // 2. ClusterExtension をセットアップ
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(StubBlockList);
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(StubProvider),
    block_list,
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

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
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. ClusterExtension をセットアップ
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(StubBlockList);
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(StubProvider),
    block_list,
    Box::new(NoopDowningProvider::new()),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id).expect("extension");

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
      if address == "node-a" && *mode == crate::core::StartupMode::Member
    )),
    "Shutdown event should be fired"
  );

  // 8. virtual_actor_count がリセットされていることを確認
  assert_eq!(ext_shared.virtual_actor_count(), 0, "virtual_actor_count should be reset after shutdown");

  // 9. blocked_members がクリアされていることを確認
  assert!(ext_shared.blocked_members().is_empty(), "blocked_members should be cleared after shutdown");
}
