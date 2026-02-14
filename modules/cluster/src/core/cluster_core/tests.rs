use alloc::{boxed::Box, string::String, vec, vec::Vec};
use core::time::Duration;

use fraktor_actor_rs::core::{
  event::stream::{
    EventStreamEvent, EventStreamShared, EventStreamSharedGeneric, EventStreamSubscriber,
    EventStreamSubscriptionGeneric, subscriber_handle,
  },
  messaging::AnyMessageGeneric,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
  time::TimerInstant,
};

use super::*;
use crate::core::{
  ClusterEvent, ClusterProviderError, ClusterProviderShared, ClusterTopology, MetricsError, StartupMode,
  TopologyUpdate,
  cluster_provider::ClusterProvider,
  grain::{GrainKey, KindRegistry, TOPIC_ACTOR_KIND},
  identity::{IdentityLookup, IdentityLookupShared, IdentitySetupError, LookupError, PidCache, PidCacheEvent},
  membership::{Gossiper, GossiperShared},
  placement::{ActivatedKind, PlacementResolution},
  pub_sub::{ClusterPubSubShared, PubSubError, cluster_pub_sub::ClusterPubSub},
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

fn apply_update_and_publish(
  core: &mut ClusterCore<NoStdToolbox>,
  event_stream: &EventStreamSharedGeneric<NoStdToolbox>,
  update: &TopologyUpdate,
) {
  let event = core.try_apply_topology(update).expect("topology apply");
  if let Some(event) = event {
    let payload = AnyMessageGeneric::new(event);
    let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    event_stream.publish(&extension_event);
  }
}

#[derive(Debug, Default)]
struct StubProvider;

impl ClusterProvider for StubProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

#[derive(Clone, Debug)]
struct FailingProvider {
  start_member_error: Option<ClusterProviderError>,
  start_client_error: Option<ClusterProviderError>,
  shutdown_error:     Option<ClusterProviderError>,
}

impl FailingProvider {
  fn member_fail(reason: impl Into<String>) -> Self {
    Self {
      start_member_error: Some(ClusterProviderError::start_member(reason)),
      start_client_error: None,
      shutdown_error:     None,
    }
  }

  fn client_fail(reason: impl Into<String>) -> Self {
    Self {
      start_member_error: None,
      start_client_error: Some(ClusterProviderError::start_client(reason)),
      shutdown_error:     None,
    }
  }

  fn shutdown_fail(reason: impl Into<String>) -> Self {
    Self {
      start_member_error: None,
      start_client_error: None,
      shutdown_error:     Some(ClusterProviderError::shutdown(reason)),
    }
  }
}

impl ClusterProvider for FailingProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    if let Some(err) = &self.start_member_error {
      return Err(err.clone());
    }
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    if let Some(err) = &self.start_client_error {
      return Err(err.clone());
    }
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    if let Some(err) = &self.shutdown_error {
      return Err(err.clone());
    }
    Ok(())
  }
}

#[derive(Debug, Clone)]
struct StubBlockListProvider {
  blocked: Vec<String>,
}

impl StubBlockListProvider {
  fn new(blocked: Vec<String>) -> Self {
    Self { blocked }
  }
}

impl BlockListProvider for StubBlockListProvider {
  fn blocked_members(&self) -> Vec<String> {
    if self.blocked.is_empty() {
      return vec![String::from("blocked-node")];
    }
    self.blocked.clone()
  }
}

#[derive(Clone, Debug, PartialEq)]
enum IdentityMode {
  Member,
  Client,
}

#[derive(Clone, Debug, PartialEq)]
struct IdentityCall {
  mode:  IdentityMode,
  kinds: Vec<String>,
}

#[derive(Clone)]
struct StubIdentityLookup {
  calls: ArcShared<NoStdMutex<Vec<IdentityCall>>>,
}

impl StubIdentityLookup {
  fn new() -> Self {
    Self { calls: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn record(&self, mode: IdentityMode, kinds: &[ActivatedKind]) {
    let mut guard = self.calls.lock();
    let mut names: Vec<String> = kinds.iter().map(|k| k.name().to_string()).collect();
    names.sort();
    guard.push(IdentityCall { mode, kinds: names });
  }

  #[allow(dead_code)]
  fn calls(&self) -> Vec<IdentityCall> {
    self.calls.lock().clone()
  }
}

impl IdentityLookup for StubIdentityLookup {
  fn setup_member(&mut self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    self.record(IdentityMode::Member, kinds);
    Ok(())
  }

  fn setup_client(&mut self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    self.record(IdentityMode::Client, kinds);
    Ok(())
  }

  fn resolve(&mut self, _key: &GrainKey, _now: u64) -> Result<PlacementResolution, LookupError> {
    Err(LookupError::NotReady)
  }
}

#[derive(Clone)]
struct StubGossiper {
  started:    ArcShared<NoStdMutex<bool>>,
  stopped:    ArcShared<NoStdMutex<bool>>,
  fail_start: bool,
  fail_stop:  bool,
}

impl StubGossiper {
  fn new() -> Self {
    Self {
      started:    ArcShared::new(NoStdMutex::new(false)),
      stopped:    ArcShared::new(NoStdMutex::new(false)),
      fail_start: false,
      fail_stop:  false,
    }
  }

  fn failing_start() -> Self {
    Self { fail_start: true, ..Self::new() }
  }

  #[allow(dead_code)]
  fn failing_stop() -> Self {
    Self { fail_stop: true, ..Self::new() }
  }

  #[allow(dead_code)]
  fn started(&self) -> bool {
    *self.started.lock()
  }

  #[allow(dead_code)]
  fn stopped(&self) -> bool {
    *self.stopped.lock()
  }
}

impl Gossiper for StubGossiper {
  fn start(&mut self) -> Result<(), &'static str> {
    if self.fail_start {
      return Err("gossip-start");
    }
    *self.started.lock() = true;
    Ok(())
  }

  fn stop(&mut self) -> Result<(), &'static str> {
    if self.fail_stop {
      return Err("gossip-stop");
    }
    *self.stopped.lock() = true;
    Ok(())
  }
}

impl Default for StubGossiper {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Clone)]
struct StubPubSub {
  started:    ArcShared<NoStdMutex<bool>>,
  stopped:    ArcShared<NoStdMutex<bool>>,
  fail_start: bool,
  fail_stop:  bool,
}

impl StubPubSub {
  fn new() -> Self {
    Self {
      started:    ArcShared::new(NoStdMutex::new(false)),
      stopped:    ArcShared::new(NoStdMutex::new(false)),
      fail_start: false,
      fail_stop:  false,
    }
  }

  fn failing_start() -> Self {
    Self { fail_start: true, ..Self::new() }
  }

  #[allow(dead_code)]
  fn failing_stop() -> Self {
    Self { fail_stop: true, ..Self::new() }
  }

  #[allow(dead_code)]
  fn started(&self) -> bool {
    *self.started.lock()
  }

  #[allow(dead_code)]
  fn stopped(&self) -> bool {
    *self.stopped.lock()
  }
}

impl ClusterPubSub<NoStdToolbox> for StubPubSub {
  fn start(&mut self) -> Result<(), PubSubError> {
    if self.fail_start {
      return Err(PubSubError::TopicAlreadyExists { topic: crate::core::pub_sub::PubSubTopic::from("pubsub-error") });
    }
    *self.started.lock() = true;
    Ok(())
  }

  fn stop(&mut self) -> Result<(), PubSubError> {
    if self.fail_stop {
      return Err(PubSubError::TopicNotFound { topic: crate::core::pub_sub::PubSubTopic::from("pubsub-error") });
    }
    *self.stopped.lock() = true;
    Ok(())
  }

  fn subscribe(
    &mut self,
    _topic: &crate::core::pub_sub::PubSubTopic,
    _subscriber: crate::core::pub_sub::PubSubSubscriber<NoStdToolbox>,
  ) -> Result<(), PubSubError> {
    Ok(())
  }

  fn unsubscribe(
    &mut self,
    _topic: &crate::core::pub_sub::PubSubTopic,
    _subscriber: crate::core::pub_sub::PubSubSubscriber<NoStdToolbox>,
  ) -> Result<(), PubSubError> {
    Ok(())
  }

  fn publish(
    &mut self,
    _request: crate::core::pub_sub::PublishRequest<NoStdToolbox>,
  ) -> Result<crate::core::pub_sub::PublishAck, PubSubError> {
    Ok(crate::core::pub_sub::PublishAck::accepted())
  }

  fn on_topology(&mut self, _update: &crate::core::TopologyUpdate) {}
}

impl Default for StubPubSub {
  fn default() -> Self {
    Self::new()
  }
}

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
  let subscriber_impl = RecordingClusterEvents::new();
  let subscriber = subscriber_handle(subscriber_impl.clone());
  let subscription = event_stream.subscribe(&subscriber);
  (subscriber_impl, subscription)
}

/// Helper wrapping an `IdentityLookup` in `IdentityLookupShared`.
fn wrap_identity_lookup<I: IdentityLookup + 'static>(lookup: I) -> IdentityLookupShared<NoStdToolbox> {
  let boxed: Box<dyn IdentityLookup> = Box::new(lookup);
  IdentityLookupShared::new(boxed)
}

/// Helper wrapping a `ClusterProvider` in `ClusterProviderShared`.
fn wrap_provider<P: ClusterProvider + 'static>(provider: P) -> ClusterProviderShared<NoStdToolbox> {
  let boxed: Box<dyn ClusterProvider> = Box::new(provider);
  ClusterProviderShared::new(boxed)
}

/// Helper wrapping a `ClusterPubSub` in `ClusterPubSubShared`.
fn wrap_pubsub<P: ClusterPubSub<NoStdToolbox> + 'static>(pubsub: P) -> ClusterPubSubShared<NoStdToolbox> {
  let boxed: Box<dyn ClusterPubSub<NoStdToolbox>> = Box::new(pubsub);
  ClusterPubSubShared::new(boxed)
}

/// Helper wrapping a `Gossiper` in `GossiperShared`.
fn wrap_gossiper<G: Gossiper + 'static>(gossiper: G) -> GossiperShared<NoStdToolbox> {
  let boxed: Box<dyn Gossiper> = Box::new(gossiper);
  GossiperShared::new(boxed)
}

fn build_core_with_config(config: &ClusterExtensionConfig) -> ClusterCore<NoStdToolbox> {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec!["blocked-node".to_string()]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());

  ClusterCore::new(
    config,
    provider,
    block_list_provider,
    event_stream,
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  )
}

#[test]
fn new_core_stores_dependencies_and_startup_params() {
  let config = ClusterExtensionConfig::new().with_advertised_address("proto://node-a").with_metrics_enabled(true);

  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec!["blocked-node".to_string()]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());

  let core = ClusterCore::new(
    &config,
    provider,
    block_list_provider.clone(),
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  // 依存がそのまま保持されていること
  let block_list_provider_dyn: ArcShared<dyn BlockListProvider> = block_list_provider.clone();
  assert!(core.block_list_provider == block_list_provider_dyn);

  assert!(core.event_stream == event_stream);

  // 構成が保持されていること
  // startup_state内部のアドレスが正しく設定されていることを確認
  assert_eq!(core.startup_address(), config.advertised_address());

  // 起動パラメータが両モードで再利用できる形で保持されること
  assert_eq!(core.startup_address(), config.advertised_address());
  assert_eq!(core.startup_address(), config.advertised_address());
}

#[test]
fn metrics_flag_reflects_config_setting() {
  let enabled_core = build_core_with_config(&ClusterExtensionConfig::new().with_metrics_enabled(true));
  assert!(enabled_core.metrics_enabled());
  let snapshot = enabled_core.metrics().unwrap();
  assert_eq!(snapshot.members(), 0);
  assert_eq!(snapshot.virtual_actors(), 0);

  let disabled_core = build_core_with_config(&ClusterExtensionConfig::new().with_metrics_enabled(false));
  assert!(!disabled_core.metrics_enabled());
  assert!(matches!(disabled_core.metrics(), Err(MetricsError::Disabled)));
}

#[test]
fn setup_member_kinds_registers_and_updates_virtual_actor_count() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![String::from("blocked-node")]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  // calls を共有して後で参照できるようにする
  let calls: ArcShared<NoStdMutex<Vec<IdentityCall>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  let identity_lookup = StubIdentityLookup { calls: calls.clone() };
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new(),
    provider,
    block_list_provider,
    event_stream,
    gossiper,
    pubsub,
    kind_registry,
    wrap_identity_lookup(identity_lookup),
  );

  core.setup_member_kinds(vec![ActivatedKind::new("worker"), ActivatedKind::new("analytics")]).unwrap();

  assert_eq!(3, core.virtual_actor_count()); // worker + analytics + topic kind

  let recorded = calls.lock().clone();
  assert_eq!(1, recorded.len());
  assert_eq!(recorded[0].mode, IdentityMode::Member);
  assert_eq!(recorded[0].kinds, vec![
    String::from("analytics"),
    String::from(TOPIC_ACTOR_KIND),
    String::from("worker"),
  ]);
}

#[test]
fn setup_client_kinds_registers_and_updates_virtual_actor_count() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  // calls を共有して後で参照できるようにする
  let calls: ArcShared<NoStdMutex<Vec<IdentityCall>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  let identity_lookup = StubIdentityLookup { calls: calls.clone() };
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new(),
    provider,
    block_list_provider,
    event_stream,
    gossiper,
    pubsub,
    kind_registry,
    wrap_identity_lookup(identity_lookup),
  );

  core.setup_client_kinds(vec![ActivatedKind::new("worker")]).unwrap();

  assert_eq!(2, core.virtual_actor_count());

  let recorded = calls.lock().clone();
  assert_eq!(1, recorded.len());
  assert_eq!(IdentityMode::Client, recorded[0].mode);
  assert_eq!(recorded[0].kinds, vec![String::from(TOPIC_ACTOR_KIND), String::from("worker")]);
}

#[test]
fn topology_event_includes_blocked_and_updates_metrics() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![String::from("blocked-a")]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_metrics_enabled(true),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  // prepare pid cache with authority that will leave
  let mut pid_cache = PidCache::new(4);
  pid_cache.put(GrainKey::new("grain-1".into()), "pid-1".into(), "node-c".into(), 0, 60);
  core.set_pid_cache(pid_cache);

  // start with one member
  core.start_member().unwrap();

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  let update = build_update(
    100,
    vec![String::from("node-a"), String::from("node-b")],
    vec![String::from("node-b")],
    vec![String::from("node-c")],
    vec![String::from("blocked-a")],
  );
  apply_update_and_publish(&mut core, &event_stream, &update);

  // member count: self + joined = 2
  let metrics = core.metrics().unwrap();
  assert_eq!(metrics.members(), 2);

  let expected_joined = vec![String::from("node-b")];
  let expected_left = vec![String::from("node-c")];
  let expected_blocked = vec![String::from("blocked-a")];
  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event,
    ClusterEvent::TopologyUpdated { update }
      if update.topology.hash() == 100
        && update.topology.joined() == &expected_joined
        && update.topology.left() == &expected_left
        && update.joined == expected_joined
        && update.left == expected_left
        && update.blocked == expected_blocked
  )));

  // pid cache invalidated for left authority
  if let Some(cache) = core.pid_cache.as_mut() {
    let events = cache.drain_events();
    assert!(events.iter().any(|e| matches!(e, PidCacheEvent::Dropped { reason, .. } if reason == "quarantine")));
  }
}

#[test]
fn topology_with_same_hash_is_suppressed() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![String::from("blocked-a")]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_metrics_enabled(true),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  core.start_member().unwrap();
  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  let update = build_update(200, vec![String::from("n2")], vec![String::from("n2")], vec![], vec![]);
  apply_update_and_publish(&mut core, &event_stream, &update);
  // same hash should be ignored
  apply_update_and_publish(&mut core, &event_stream, &update);

  let events = subscriber_impl.events();
  let topology_events: Vec<_> =
    events.iter().filter(|event| matches!(event, ClusterEvent::TopologyUpdated { .. })).collect();
  assert_eq!(1, topology_events.len());
}

#[test]
fn multi_node_topology_flow_updates_metrics_and_pid_cache() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![String::from("blocked-b")]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_metrics_enabled(true),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  core.start_member().unwrap();
  let mut pid_cache = PidCache::new(8);
  pid_cache.put(GrainKey::new("grain-1".into()), "pid-1".into(), "n2".into(), 0, 60);
  pid_cache.put(GrainKey::new("grain-2".into()), "pid-2".into(), "n3".into(), 0, 60);
  core.set_pid_cache(pid_cache);

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  // node n2 joins, n3 leaves
  let update = build_update(300, vec![String::from("n2")], vec![String::from("n2")], vec![String::from("n3")], vec![
    String::from("blocked-b"),
  ]);
  apply_update_and_publish(&mut core, &event_stream, &update);

  // members: start 1 -> +1 -1 =1
  let metrics = core.metrics().unwrap();
  assert_eq!(metrics.members(), 1);

  // pid cache should have dropped n3 entries
  if let Some(cache) = core.pid_cache.as_mut() {
    let events = cache.drain_events();
    assert!(events.iter().any(|e| matches!(e, PidCacheEvent::Dropped { reason, .. } if reason == "quarantine")));
  }

  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event,
    ClusterEvent::TopologyUpdated { update }
      if update.topology.hash() == 300
        && update.topology.joined().contains(&"n2".to_string())
        && update.topology.left().contains(&"n3".to_string())
        && update.joined.contains(&"n2".to_string())
        && update.left.contains(&"n3".to_string())
        && update.blocked.contains(&"blocked-b".to_string())
  )));
}

#[test]
fn start_member_emits_startup_event_and_sets_mode() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://member").with_metrics_enabled(true),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  core.setup_member_kinds(vec![ActivatedKind::new("worker"), ActivatedKind::new("analytics")]).unwrap();
  core.start_member().unwrap();

  let metrics = core.metrics().unwrap();
  assert_eq!(metrics.members(), 1);
  assert_eq!(metrics.virtual_actors(), 3);

  let events = subscriber_impl.events();
  assert!(
    events.contains(&ClusterEvent::Startup { address: String::from("proto://member"), mode: StartupMode::Member })
  );

  // BlockList がキャッシュされることを確認
  assert_eq!(core.blocked_members(), &[String::from("blocked-node")]);
}

#[test]
fn start_member_failure_emits_startup_failed() {
  let provider = wrap_provider(FailingProvider::member_fail("boom"));
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://member"),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  let result = core.start_member();
  assert!(result.is_err());

  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event,
    ClusterEvent::StartupFailed { address, mode, reason }
      if address == "proto://member" && *mode == StartupMode::Member && reason == "boom"
  )));
}

#[test]
fn start_client_emits_startup_event_and_sets_mode() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://client"),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  core.start_client().unwrap();

  let events = subscriber_impl.events();
  assert!(
    events.contains(&ClusterEvent::Startup { address: String::from("proto://client"), mode: StartupMode::Client })
  );
}

#[test]
fn start_client_failure_emits_startup_failed() {
  let provider = wrap_provider(FailingProvider::client_fail("boom"));
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://client"),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  let result = core.start_client();
  assert!(result.is_err());

  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event,
    ClusterEvent::StartupFailed { address, mode, reason }
      if address == "proto://client" && *mode == StartupMode::Client && reason == "boom"
  )));
}

#[test]
fn start_member_fails_when_gossip_start_fails() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::failing_start());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://member"),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  let result = core.start_member();
  assert!(result.is_err());
  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event,
    ClusterEvent::StartupFailed { address, mode, reason }
      if address == "proto://member" && *mode == StartupMode::Member && reason == "gossip-start"
  )));
}

#[test]
fn start_member_fails_when_pubsub_start_fails() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::failing_start());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://member"),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  let result = core.start_member();
  assert!(result.is_err());
  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event,
    ClusterEvent::StartupFailed { address, mode, reason }
      if address == "proto://member" && *mode == StartupMode::Member && reason.contains("pubsub")
  )));
}

#[test]
fn shutdown_stops_pubsub_then_gossip() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper_stopped: ArcShared<NoStdMutex<bool>> = ArcShared::new(NoStdMutex::new(false));
  let pubsub_stopped: ArcShared<NoStdMutex<bool>> = ArcShared::new(NoStdMutex::new(false));
  let gossiper = wrap_gossiper(StubGossiper {
    started:    ArcShared::new(NoStdMutex::new(false)),
    stopped:    gossiper_stopped.clone(),
    fail_start: false,
    fail_stop:  false,
  });
  let pubsub = wrap_pubsub(StubPubSub {
    started:    ArcShared::new(NoStdMutex::new(false)),
    stopped:    pubsub_stopped.clone(),
    fail_start: false,
    fail_stop:  false,
  });
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://member"),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  core.start_member().unwrap();
  core.shutdown(true).unwrap();

  assert!(*pubsub_stopped.lock());
  assert!(*gossiper_stopped.lock());
}

#[test]
fn shutdown_resets_virtual_actor_count_and_emits_event() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://member"),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  core.setup_member_kinds(vec![ActivatedKind::new("worker")]).unwrap();
  core.start_member().unwrap();

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  core.shutdown(true).unwrap();

  assert_eq!(0, core.virtual_actor_count());
  assert!(core.blocked_members().is_empty());
  let events = subscriber_impl.events();
  assert!(
    events.contains(&ClusterEvent::Shutdown { address: String::from("proto://member"), mode: StartupMode::Member })
  );
}

#[test]
fn shutdown_failure_emits_shutdown_failed() {
  let provider = wrap_provider(FailingProvider::shutdown_fail("stop-error"));
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://member"),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  core.start_member().ok();

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  let result = core.shutdown(true);
  assert!(result.is_err());

  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event,
    ClusterEvent::ShutdownFailed { address, mode, reason }
      if address == "proto://member" && *mode == StartupMode::Member && reason == "stop-error"
  )));
}

// ====================================================================
// タスク 5.1: metrics 無効時の挙動と EventStream 出力を検証
// 要件5.2 をカバー
// ====================================================================

/// metrics が無効構成のときに MetricsError::Disabled を返すことを検証
#[test]
fn metrics_disabled_returns_error() {
  let config = ClusterExtensionConfig::new().with_metrics_enabled(false);
  let core = build_core_with_config(&config);
  assert!(!core.metrics_enabled());
  assert!(matches!(core.metrics(), Err(MetricsError::Disabled)));
}

/// metrics 無効時でも Startup イベントは EventStream に発火されることを検証
#[test]
fn metrics_disabled_still_emits_startup_event() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://member").with_metrics_enabled(false),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  // EventStream subscriber を登録
  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  // metrics は無効
  assert!(matches!(core.metrics(), Err(MetricsError::Disabled)));

  // start_member を実行
  core.start_member().unwrap();

  // Startup イベントが発火されたことを確認
  let events = subscriber_impl.events();
  assert!(
    events.contains(&ClusterEvent::Startup { address: String::from("proto://member"), mode: StartupMode::Member })
  );
}

/// metrics 無効時でも TopologyUpdated イベントは EventStream に発火されることを検証
#[test]
fn metrics_disabled_still_emits_topology_updated_event() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![String::from("blocked-x")]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://member").with_metrics_enabled(false),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  // start_member を実行
  core.start_member().unwrap();

  // EventStream subscriber を登録
  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  // metrics は無効
  assert!(matches!(core.metrics(), Err(MetricsError::Disabled)));

  // トポロジ更新を行う
  let update =
    build_update(7000, vec![String::from("node-y")], vec![String::from("node-y")], vec![], vec![String::from(
      "blocked-x",
    )]);
  apply_update_and_publish(&mut core, &event_stream, &update);

  // TopologyUpdated イベントが発火されたことを確認
  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event,
    ClusterEvent::TopologyUpdated { update }
      if update.topology.hash() == 7000
        && update.joined == vec![String::from("node-y")]
        && update.left.is_empty()
        && update.blocked.contains(&String::from("blocked-x"))
  )));
}

/// metrics 無効時でも Shutdown イベントは EventStream に発火されることを検証
#[test]
fn metrics_disabled_still_emits_shutdown_event() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://member").with_metrics_enabled(false),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  // start_member を実行
  core.start_member().unwrap();

  // EventStream subscriber を登録
  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  // metrics は無効
  assert!(matches!(core.metrics(), Err(MetricsError::Disabled)));

  // shutdown を実行
  core.shutdown(true).unwrap();

  // Shutdown イベントが発火されたことを確認
  let events = subscriber_impl.events();
  assert!(
    events.contains(&ClusterEvent::Shutdown { address: String::from("proto://member"), mode: StartupMode::Member })
  );
}

/// metrics 無効時でも全てのクラスタイベント（Startup/TopologyUpdated/Shutdown）が
/// EventStream に継続して発火されることを包括的に検証
#[test]
fn metrics_disabled_full_lifecycle_events_continue() {
  let provider = wrap_provider(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![String::from("blocked-z")]));
  let event_stream = EventStreamShared::default();
  let kind_registry = KindRegistry::new();
  let identity_lookup = wrap_identity_lookup(StubIdentityLookup::new());
  let gossiper = wrap_gossiper(StubGossiper::new());
  let pubsub = wrap_pubsub(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://full-lifecycle").with_metrics_enabled(false),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  // EventStream subscriber を登録
  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  // metrics が無効であることを確認
  assert!(!core.metrics_enabled());
  assert!(matches!(core.metrics(), Err(MetricsError::Disabled)));

  // 1. start_member
  core.start_member().unwrap();

  // 2. トポロジ更新を複数回
  let update1 =
    build_update(8001, vec![String::from("node-1")], vec![String::from("node-1")], vec![], vec![String::from(
      "blocked-z",
    )]);
  apply_update_and_publish(&mut core, &event_stream, &update1);

  let update2 =
    build_update(8002, vec![String::from("node-2")], vec![String::from("node-2")], vec![String::from("node-1")], vec![
      String::from("blocked-z"),
    ]);
  apply_update_and_publish(&mut core, &event_stream, &update2);

  // 3. shutdown
  core.shutdown(true).unwrap();

  // すべてのイベントが発火されたことを確認
  let events = subscriber_impl.events();

  // Startup イベント
  assert!(events.contains(&ClusterEvent::Startup {
    address: String::from("proto://full-lifecycle"),
    mode:    StartupMode::Member,
  }));

  // 最初の TopologyUpdated イベント
  assert!(events.iter().any(|event| matches!(event,
    ClusterEvent::TopologyUpdated { update }
      if update.topology.hash() == 8001 && update.joined.contains(&String::from("node-1"))
  )));

  // 2番目の TopologyUpdated イベント
  assert!(events.iter().any(|event| matches!(event,
    ClusterEvent::TopologyUpdated { update }
      if update.topology.hash() == 8002
        && update.joined.contains(&String::from("node-2"))
        && update.left.contains(&String::from("node-1"))
  )));

  // Shutdown イベント
  assert!(events.contains(&ClusterEvent::Shutdown {
    address: String::from("proto://full-lifecycle"),
    mode:    StartupMode::Member,
  }));

  // metrics は終始 Disabled のまま
  assert!(matches!(core.metrics(), Err(MetricsError::Disabled)));
}
