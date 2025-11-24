use alloc::{string::String, vec, vec::Vec};

use fraktor_actor_rs::core::event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use super::*;
use crate::core::{
  ActivatedKind, ClusterEvent, ClusterProviderError, ClusterPubSub, ClusterTopology, Gossiper, IdentityLookup,
  IdentitySetupError, KindRegistry, MetricsError, PidCacheEvent, StartupMode, TOPIC_ACTOR_KIND, grain_key::GrainKey,
  pid_cache::PidCache, pub_sub_error::PubSubError,
};

#[derive(Debug, Default)]
struct StubProvider;

impl ClusterProvider for StubProvider {
  fn start_member(&self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&self, _graceful: bool) -> Result<(), ClusterProviderError> {
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
  fn start_member(&self) -> Result<(), ClusterProviderError> {
    if let Some(err) = &self.start_member_error {
      return Err(err.clone());
    }
    Ok(())
  }

  fn start_client(&self) -> Result<(), ClusterProviderError> {
    if let Some(err) = &self.start_client_error {
      return Err(err.clone());
    }
    Ok(())
  }

  fn shutdown(&self, _graceful: bool) -> Result<(), ClusterProviderError> {
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

  fn calls(&self) -> Vec<IdentityCall> {
    self.calls.lock().clone()
  }
}

impl IdentityLookup for StubIdentityLookup {
  fn setup_member(&self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    self.record(IdentityMode::Member, kinds);
    Ok(())
  }

  fn setup_client(&self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    self.record(IdentityMode::Client, kinds);
    Ok(())
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

  fn stopped(&self) -> bool {
    *self.stopped.lock()
  }
}

impl Gossiper for StubGossiper {
  fn start(&self) -> Result<(), &'static str> {
    if self.fail_start {
      return Err("gossip-start");
    }
    *self.started.lock() = true;
    Ok(())
  }

  fn stop(&self) -> Result<(), &'static str> {
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

  fn stopped(&self) -> bool {
    *self.stopped.lock()
  }
}

impl ClusterPubSub for StubPubSub {
  fn start(&self) -> Result<(), PubSubError> {
    if self.fail_start {
      return Err(PubSubError::TopicAlreadyExists { topic: String::from("pubsub-error") });
    }
    *self.started.lock() = true;
    Ok(())
  }

  fn stop(&self) -> Result<(), PubSubError> {
    if self.fail_stop {
      return Err(PubSubError::TopicNotFound { topic: String::from("pubsub-error") });
    }
    *self.stopped.lock() = true;
    Ok(())
  }
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
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    if let EventStreamEvent::Extension { name, payload } = event {
      if name == "cluster" {
        if let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>() {
          self.events.lock().push(cluster_event.clone());
        }
      }
    }
  }
}

fn build_core_with_config(config: &ClusterExtensionConfig) -> ClusterCore<NoStdToolbox> {
  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec!["blocked-node".to_string()]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup: ArcShared<dyn IdentityLookup> = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());

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

  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec!["blocked-node".to_string()]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup: ArcShared<dyn IdentityLookup> = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());

  let core = ClusterCore::new(
    &config,
    provider.clone(),
    block_list_provider.clone(),
    event_stream.clone(),
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup,
  );

  // 依存がそのまま保持されていること
  let provider_dyn: ArcShared<dyn ClusterProvider> = provider.clone();
  assert!(core.provider == provider_dyn);

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
  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![String::from("blocked-node")]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new(),
    provider,
    block_list_provider,
    event_stream,
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup.clone(),
  );

  core.setup_member_kinds(vec![ActivatedKind::new("worker"), ActivatedKind::new("analytics")]).unwrap();

  assert_eq!(3, core.virtual_actor_count()); // worker + analytics + topic kind

  let recorded = identity_lookup.calls();
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
  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new(),
    provider,
    block_list_provider,
    event_stream,
    gossiper,
    pubsub,
    kind_registry,
    identity_lookup.clone(),
  );

  core.setup_client_kinds(vec![ActivatedKind::new("worker")]).unwrap();

  assert_eq!(2, core.virtual_actor_count());

  let recorded = identity_lookup.calls();
  assert_eq!(1, recorded.len());
  assert_eq!(IdentityMode::Client, recorded[0].mode);
  assert_eq!(recorded[0].kinds, vec![String::from(TOPIC_ACTOR_KIND), String::from("worker")]);
}

#[test]
fn topology_event_includes_blocked_and_updates_metrics() {
  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![String::from("blocked-a")]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());
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

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let topology = ClusterTopology::new(100, vec![String::from("node-b")], vec![String::from("node-c")]);
  core.on_topology(&topology);

  // member count: 1 +1 -1 =1
  let metrics = core.metrics().unwrap();
  assert_eq!(metrics.members(), 1);

  let expected_joined = vec![String::from("node-b")];
  let expected_left = vec![String::from("node-c")];
  let expected_blocked = vec![String::from("blocked-a")];
  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event,
    ClusterEvent::TopologyUpdated { topology, joined, left, blocked }
      if topology.hash() == 100
        && topology.joined() == &expected_joined
        && topology.left() == &expected_left
        && joined == &expected_joined
        && left == &expected_left
        && blocked == &expected_blocked
  )));

  // pid cache invalidated for left authority
  if let Some(cache) = core.pid_cache.as_mut() {
    let events = cache.drain_events();
    assert!(events.iter().any(|e| matches!(e, PidCacheEvent::Dropped { reason, .. } if reason == "quarantine")));
  }
}

#[test]
fn topology_with_same_hash_is_suppressed() {
  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![String::from("blocked-a")]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());
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
  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let topology = ClusterTopology::new(200, vec![String::from("n2")], vec![]);
  core.on_topology(&topology);
  // same hash should be ignored
  core.on_topology(&topology);

  let events = subscriber_impl.events();
  let topology_events: Vec<_> =
    events.iter().filter(|event| matches!(event, ClusterEvent::TopologyUpdated { .. })).collect();
  assert_eq!(1, topology_events.len());
}

#[test]
fn multi_node_topology_flow_updates_metrics_and_pid_cache() {
  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![String::from("blocked-b")]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());
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

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  // node n2 joins, n3 leaves
  let topology = ClusterTopology::new(300, vec![String::from("n2")], vec![String::from("n3")]);
  core.on_topology(&topology);

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
    ClusterEvent::TopologyUpdated { topology, joined, left, blocked }
      if topology.hash() == 300
        && topology.joined().contains(&"n2".to_string())
        && topology.left().contains(&"n3".to_string())
        && joined.contains(&"n2".to_string())
        && left.contains(&"n3".to_string())
        && blocked.contains(&"blocked-b".to_string())
  )));
}

#[test]
fn start_member_emits_startup_event_and_sets_mode() {
  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());
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

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

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
  let provider = ArcShared::new(FailingProvider::member_fail("boom"));
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());
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

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

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
  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());
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

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  core.start_client().unwrap();

  let events = subscriber_impl.events();
  assert!(
    events.contains(&ClusterEvent::Startup { address: String::from("proto://client"), mode: StartupMode::Client })
  );
}

#[test]
fn start_client_failure_emits_startup_failed() {
  let provider = ArcShared::new(FailingProvider::client_fail("boom"));
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());
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

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

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
  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::failing_start());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());
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

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

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
  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::failing_start());
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

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

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
  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper = ArcShared::new(StubGossiper::new());
  let pubsub = ArcShared::new(StubPubSub::new());
  let mut core = ClusterCore::new(
    &ClusterExtensionConfig::new().with_advertised_address("proto://member"),
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper.clone(),
    pubsub.clone(),
    kind_registry,
    identity_lookup,
  );

  core.start_member().unwrap();
  core.shutdown(true).unwrap();

  assert!(pubsub.stopped());
  assert!(gossiper.stopped());
}

#[test]
fn shutdown_resets_virtual_actor_count_and_emits_event() {
  let provider = ArcShared::new(StubProvider);
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());
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

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

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
  let provider = ArcShared::new(FailingProvider::shutdown_fail("stop-error"));
  let block_list_provider = ArcShared::new(StubBlockListProvider::new(vec![]));
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let kind_registry = KindRegistry::new();
  let identity_lookup = ArcShared::new(StubIdentityLookup::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(StubGossiper::new());
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(StubPubSub::new());
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

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let result = core.shutdown(true);
  assert!(result.is_err());

  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event,
    ClusterEvent::ShutdownFailed { address, mode, reason }
      if address == "proto://member" && *mode == StartupMode::Member && reason == "stop-error"
  )));
}
