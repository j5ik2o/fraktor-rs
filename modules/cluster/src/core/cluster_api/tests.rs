use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_actor_rs::core::{
  actor::{
    Actor, Pid,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::{ActorRefGeneric, ActorRefSender, ActorRefSenderSharedGeneric, SendOutcome},
  },
  error::ActorError,
  event::stream::{
    EventStreamEvent, EventStreamSharedGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric,
    subscriber_handle,
  },
  extension::ExtensionInstallers,
  messaging::AnyMessageGeneric,
  props::PropsGeneric,
  scheduler::{
    SchedulerConfig, SchedulerSharedGeneric,
    tick_driver::{ManualTestDriver, TickDriverConfig},
  },
  system::{
    ActorSystemConfigGeneric, ActorSystemGeneric,
    provider::{ActorRefProvider, ActorRefProviderSharedGeneric},
  },
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::{ArcShared, SharedAccess},
  time::TimerInstant,
};

use crate::core::{
  ClusterApiError, ClusterApiGeneric, ClusterEvent, ClusterEventType, ClusterExtensionConfig, ClusterExtensionGeneric,
  ClusterExtensionInstaller, ClusterRequestError, ClusterResolveError, ClusterSubscriptionInitialStateMode,
  ClusterTopology, MetricsError, TopologyUpdate,
  cluster_provider::{ClusterProvider, NoopClusterProvider},
  downing_provider::DowningProvider,
  grain::{GRAIN_EVENT_STREAM_NAME, GrainEvent, GrainKey},
  identity::{ClusterIdentity, IdentityLookup, IdentitySetupError, LookupError, NoopIdentityLookup},
  placement::{ActivatedKind, PlacementDecision, PlacementEvent, PlacementLocality, PlacementResolution},
};

#[test]
fn try_from_system_fails_when_extension_missing() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  match ClusterApiGeneric::try_from_system(&system) {
    | Ok(_) => panic!("extension should be missing"),
    | Err(err) => assert_eq!(err, ClusterApiError::ExtensionNotInstalled),
  }
}

#[test]
fn try_from_system_returns_existing_extension() {
  let (system, _ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));

  let first = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let second = ClusterApiGeneric::try_from_system(&system).expect("cluster api");

  assert!(ArcShared::ptr_eq(&first.extension, &second.extension));
}

#[test]
fn get_fails_when_cluster_not_started() {
  let (system, _ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let err = api.get(&identity).expect_err("not started");
  assert_eq!(err, ClusterResolveError::ClusterNotStarted);
}

#[test]
fn get_fails_when_kind_not_registered() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let err = api.get(&identity).expect_err("kind not registered");
  assert_eq!(err, ClusterResolveError::KindNotRegistered { kind: "user".to_string() });
}

#[test]
fn get_fails_on_invalid_pid_format() {
  let (system, ext) = build_system_with_extension(|| Box::new(InvalidIdentityLookup));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let err = api.get(&identity).expect_err("invalid pid");
  assert!(matches!(err, ClusterResolveError::InvalidPidFormat { .. }));
}

#[test]
fn get_returns_lookup_pending_when_resolution_pending() {
  let (system, ext) = build_system_with_extension(|| Box::new(PendingIdentityLookup::new()));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let err = api.get(&identity).expect_err("pending");
  assert_eq!(err, ClusterResolveError::LookupPending);
}

#[test]
fn get_resolves_actor_ref_for_registered_kind() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let actor_ref = api.get(&identity).expect("resolved actor ref");
  assert_eq!(actor_ref.pid(), Pid::new(1, 0));
}

#[test]
fn grain_metrics_returns_disabled_when_metrics_not_enabled() {
  let (_system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  assert_eq!(ext.grain_metrics(), Err(MetricsError::Disabled));
}

#[test]
fn get_publishes_activation_events_and_updates_metrics() {
  let (system, ext) = build_system_with_extension_config(|| Box::new(EventfulIdentityLookup::new("node1:8080")), true);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let event_stream = system.event_stream();
  let (recorder, _subscription) = subscribe_grain_events(&event_stream);

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let _ = api.get(&identity).expect("resolved actor ref");

  let events = recorder.events();
  assert!(events.iter().any(|event| matches!(event, GrainEvent::ActivationCreated { .. })));
  assert!(events.iter().any(|event| matches!(event, GrainEvent::ActivationPassivated { .. })));

  let metrics = ext.grain_metrics().expect("metrics");
  assert_eq!(metrics.activations_created(), 1);
  assert_eq!(metrics.activations_passivated(), 1);
}

#[test]
fn request_returns_error_when_lookup_fails() {
  let (system, ext) = build_system_with_extension(|| Box::new(NoopIdentityLookup::new()));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  match api.request(&identity, AnyMessageGeneric::new(()), None) {
    | Ok(_) => panic!("lookup should fail"),
    | Err(err) => assert_eq!(err, ClusterRequestError::ResolveFailed(ClusterResolveError::LookupFailed)),
  }
}

#[test]
fn request_returns_ok_without_timeout() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let response = api.request(&identity, AnyMessageGeneric::new(()), None).expect("request ok");

  assert!(!response.future().with_read(|inner| inner.is_ready()));
}

#[test]
fn request_future_completes_with_timeout_payload() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let future =
    api.request_future(&identity, AnyMessageGeneric::new(()), Some(Duration::from_millis(1))).expect("request");

  assert!(!future.with_read(|inner| inner.is_ready()));

  run_scheduler(&system, Duration::from_millis(1));

  let result = future.with_write(|inner| inner.try_take()).expect("timeout payload");
  assert!(result.is_err(), "expect timeout error");
  let ask_error = result.unwrap_err();
  assert_eq!(ask_error, fraktor_actor_rs::core::messaging::AskError::Timeout);
}

#[test]
fn down_delegates_to_cluster_provider() {
  let downed_provider: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  let downed_strategy: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  let downed_for_provider = downed_provider.clone();
  let downed_for_strategy = downed_strategy.clone();

  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let cluster_config = ClusterExtensionConfig::new().with_advertised_address("node1:8080");
  let cluster_installer =
    ClusterExtensionInstaller::new(cluster_config, move |_event_stream, _block_list, _address| {
      Box::new(RecordingDownProvider { downed: downed_for_provider.clone() })
    })
    .with_downing_provider_factory(move || Box::new(RecordingDowningProvider { downed: downed_for_strategy.clone() }))
    .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler_config)
    .with_tick_driver(tick_driver)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(|system: &ActorSystemGeneric<NoStdToolbox>| {
      let provider = ActorRefProviderSharedGeneric::new(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&provider)
    });
  let props = PropsGeneric::from_fn(|| TestGuardian);
  let system = ActorSystemGeneric::new_with_config(&props, &config).expect("build system");
  let extension =
    system.extended().extension_by_type::<ClusterExtensionGeneric<NoStdToolbox>>().expect("cluster extension");
  extension.start_member().expect("start member");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  api.down("node2:8080").expect("down");

  assert_eq!(downed_strategy.lock().clone(), vec![String::from("node2:8080")]);
  assert_eq!(downed_provider.lock().clone(), vec![String::from("node2:8080")]);
}

#[test]
fn join_and_leave_delegate_to_cluster_provider() {
  let joined_provider: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  let left_provider: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  let joined_for_provider = joined_provider.clone();
  let left_for_provider = left_provider.clone();

  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let cluster_config = ClusterExtensionConfig::new().with_advertised_address("node1:8080");
  let cluster_installer =
    ClusterExtensionInstaller::new(cluster_config, move |_event_stream, _block_list, _address| {
      Box::new(RecordingMembershipProvider { joined: joined_for_provider.clone(), left: left_for_provider.clone() })
    })
    .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler_config)
    .with_tick_driver(tick_driver)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(|system: &ActorSystemGeneric<NoStdToolbox>| {
      let provider = ActorRefProviderSharedGeneric::new(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&provider)
    });
  let props = PropsGeneric::from_fn(|| TestGuardian);
  let system = ActorSystemGeneric::new_with_config(&props, &config).expect("build system");
  let extension =
    system.extended().extension_by_type::<ClusterExtensionGeneric<NoStdToolbox>>().expect("cluster extension");
  extension.start_member().expect("start member");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  api.join("node2:8080").expect("join");
  api.leave("node2:8080").expect("leave");

  assert_eq!(joined_provider.lock().clone(), vec![String::from("node2:8080")]);
  assert_eq!(left_provider.lock().clone(), vec![String::from("node2:8080")]);
}

#[test]
fn subscribe_and_unsubscribe_control_event_stream_registration() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = subscriber_handle(recorder.clone());
  let subscription =
    api.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsEvents, &[ClusterEventType::TopologyUpdated]);

  let first = build_topology_update(1, vec![String::from("node2:8080")], Vec::new());
  extension.on_topology(&first);
  assert!(!recorder.events().is_empty());

  recorder.clear();
  api.unsubscribe(subscription.id());

  let second = build_topology_update(2, vec![String::from("node3:8080")], Vec::new());
  extension.on_topology(&second);
  assert!(recorder.events().is_empty());
}

#[test]
fn subscribe_snapshot_mode_sends_current_cluster_state_first() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let first = build_topology_update(1, vec![String::from("node2:8080")], Vec::new());
  extension.on_topology(&first);
  let second = build_topology_update(2, vec![String::from("node3:8080")], Vec::new());
  extension.on_topology(&second);

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = subscriber_handle(recorder.clone());
  let _subscription =
    api.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsSnapshot, &[ClusterEventType::TopologyUpdated]);

  let events = recorder.events();
  assert_eq!(events.len(), 1);
  assert!(matches!(
    &events[0],
    ClusterEvent::CurrentClusterState { state, observed_at }
      if observed_at.ticks() == 2
        && state.members.iter().map(|record| record.authority.as_str()).collect::<Vec<_>>() == vec![
          "node1:8080",
          "node3:8080",
        ]
        && state.unreachable.is_empty()
        && state.seen_by.is_empty()
        && state.leader.as_deref() == Some("node1:8080")
  ));

  recorder.clear();
  let third = build_topology_update(3, vec![String::from("node4:8080")], Vec::new());
  extension.on_topology(&third);

  let replayed = recorder.events();
  assert_eq!(replayed.len(), 1);
  assert!(matches!(&replayed[0], ClusterEvent::TopologyUpdated { update } if update.topology.hash() == 3));
}

#[test]
fn subscribe_snapshot_mode_sends_self_member_before_topology_events() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = subscriber_handle(recorder.clone());
  let _subscription =
    api.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsSnapshot, &[ClusterEventType::TopologyUpdated]);

  let events = recorder.events();
  assert_eq!(events.len(), 1);
  assert!(matches!(
    &events[0],
    ClusterEvent::CurrentClusterState { state, .. }
      if state.members.iter().map(|record| record.authority.as_str()).collect::<Vec<_>>() == vec!["node1:8080"]
        && state.unreachable.is_empty()
        && state.seen_by.is_empty()
        && state.leader.as_deref() == Some("node1:8080")
  ));
}

#[test]
fn subscribe_snapshot_mode_keeps_current_cluster_state_first_when_topology_updates_after_subscribe() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let first = build_topology_update(1, vec![String::from("node2:8080")], Vec::new());
  extension.on_topology(&first);

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = subscriber_handle(recorder.clone());
  let _subscription =
    api.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsSnapshot, &[ClusterEventType::TopologyUpdated]);

  let second = build_topology_update(2, vec![String::from("node3:8080")], Vec::new());
  extension.on_topology(&second);

  let events = recorder.events();
  assert_eq!(events.len(), 2);
  assert!(matches!(
    &events[0],
    ClusterEvent::CurrentClusterState { state, observed_at }
      if observed_at.ticks() == 1
        && state.members.iter().map(|record| record.authority.as_str()).collect::<Vec<_>>() == vec![
          "node1:8080",
          "node2:8080",
        ]
  ));
  assert!(matches!(&events[1], ClusterEvent::TopologyUpdated { update } if update.topology.hash() == 2));
}

#[test]
fn subscribe_no_replay_skips_buffered_cluster_events() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let first = build_topology_update(1, vec![String::from("node2:8080")], Vec::new());
  extension.on_topology(&first);

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = subscriber_handle(recorder.clone());
  let _subscription = api.subscribe_no_replay(&subscriber, &[ClusterEventType::TopologyUpdated]);
  assert!(recorder.events().is_empty());

  let second = build_topology_update(2, vec![String::from("node3:8080")], Vec::new());
  extension.on_topology(&second);

  let events = recorder.events();
  assert_eq!(events.len(), 1);
  assert!(matches!(&events[0], ClusterEvent::TopologyUpdated { update } if update.topology.hash() == 2));
}

#[test]
#[should_panic(expected = "at least one cluster event type is required")]
fn subscribe_panics_when_event_type_filter_is_empty() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let recorder = RecordingClusterEvents::new();
  let subscriber = subscriber_handle(recorder);

  let _ = api.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsEvents, &[]);
}

fn run_scheduler(system: &ActorSystemGeneric<NoStdToolbox>, duration: Duration) {
  let scheduler: SchedulerSharedGeneric<NoStdToolbox> = system.state().scheduler();
  let resolution = scheduler.with_read(|inner| inner.config().resolution());
  let resolution_ns = resolution.as_nanos().max(1);
  let ticks = duration.as_nanos().div_ceil(resolution_ns).max(1);
  let now = TimerInstant::from_ticks(ticks as u64, resolution);
  scheduler.with_write(|inner| {
    let _ = inner.run_due(now);
  });
}

fn build_system_with_extension<F>(
  identity_lookup_factory: F,
) -> (ActorSystemGeneric<NoStdToolbox>, ArcShared<ClusterExtensionGeneric<NoStdToolbox>>)
where
  F: Fn() -> Box<dyn IdentityLookup> + Send + Sync + 'static, {
  build_system_with_extension_config(identity_lookup_factory, false)
}

fn build_system_with_extension_config<F>(
  identity_lookup_factory: F,
  metrics_enabled: bool,
) -> (ActorSystemGeneric<NoStdToolbox>, ArcShared<ClusterExtensionGeneric<NoStdToolbox>>)
where
  F: Fn() -> Box<dyn IdentityLookup> + Send + Sync + 'static, {
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let cluster_config =
    ClusterExtensionConfig::new().with_advertised_address("node1:8080").with_metrics_enabled(metrics_enabled);
  let cluster_installer = ClusterExtensionInstaller::new(cluster_config, |_event_stream, _block_list, _address| {
    Box::new(NoopClusterProvider::new())
  })
  .with_identity_lookup_factory(identity_lookup_factory);
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler_config)
    .with_tick_driver(tick_driver)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(|system: &ActorSystemGeneric<NoStdToolbox>| {
      let provider = ActorRefProviderSharedGeneric::new(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&provider)
    });
  let props = PropsGeneric::from_fn(|| TestGuardian);
  let system = ActorSystemGeneric::new_with_config(&props, &config).expect("build system");
  let extension =
    system.extended().extension_by_type::<ClusterExtensionGeneric<NoStdToolbox>>().expect("cluster extension");
  (system, extension)
}

#[derive(Clone)]
struct RecordingGrainEvents {
  events: ArcShared<NoStdMutex<Vec<GrainEvent>>>,
}

impl RecordingGrainEvents {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<GrainEvent> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber<NoStdToolbox> for RecordingGrainEvents {
  fn on_event(&mut self, event: &EventStreamEvent<NoStdToolbox>) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == GRAIN_EVENT_STREAM_NAME
      && let Some(grain_event) = payload.payload().downcast_ref::<GrainEvent>()
    {
      self.events.lock().push(grain_event.clone());
    }
  }
}

fn subscribe_grain_events(
  event_stream: &EventStreamSharedGeneric<NoStdToolbox>,
) -> (RecordingGrainEvents, EventStreamSubscriptionGeneric<NoStdToolbox>) {
  let recorder = RecordingGrainEvents::new();
  let subscriber = subscriber_handle(recorder.clone());
  let subscription = event_stream.subscribe(&subscriber);
  (recorder, subscription)
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

  fn clear(&self) {
    self.events.lock().clear();
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

fn build_topology_update(version: u64, joined: Vec<String>, left: Vec<String>) -> TopologyUpdate {
  let topology = ClusterTopology::new(version, joined.clone(), left.clone(), Vec::new());
  let mut members = vec![String::from("node1:8080")];
  for authority in &joined {
    if !members.contains(authority) {
      members.push(authority.clone());
    }
  }
  members.retain(|authority| !left.contains(authority));
  TopologyUpdate::new(
    topology,
    members,
    joined,
    left,
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(version, Duration::from_secs(1)),
  )
}

struct TestGuardian;

impl Actor<NoStdToolbox> for TestGuardian {
  fn receive(
    &mut self,
    _context: &mut fraktor_actor_rs::core::actor::ActorContextGeneric<'_, NoStdToolbox>,
    _message: fraktor_actor_rs::core::messaging::AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), fraktor_actor_rs::core::error::ActorError> {
    Ok(())
  }
}

struct StaticIdentityLookup {
  authority: String,
}

impl StaticIdentityLookup {
  fn new(authority: &str) -> Self {
    Self { authority: authority.to_string() }
  }
}

impl IdentityLookup for StaticIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    let pid = format!("{}::{}", self.authority, key.value());
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid,
    })
  }
}

struct EventfulIdentityLookup {
  authority: String,
  events:    Vec<PlacementEvent>,
}

impl EventfulIdentityLookup {
  fn new(authority: &str) -> Self {
    Self { authority: authority.to_string(), events: Vec::new() }
  }
}

impl IdentityLookup for EventfulIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    let pid = format!("{}::{}", self.authority, key.value());
    self.events.push(PlacementEvent::Activated {
      key:         key.clone(),
      pid:         pid.clone(),
      observed_at: now,
    });
    self.events.push(PlacementEvent::Passivated { key: key.clone(), observed_at: now });
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid,
    })
  }

  fn drain_events(&mut self) -> Vec<PlacementEvent> {
    core::mem::take(&mut self.events)
  }
}

struct InvalidIdentityLookup;

impl IdentityLookup for InvalidIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: "invalid".to_string(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid:      "invalid_pid".to_string(),
    })
  }
}

struct PendingIdentityLookup;

impl PendingIdentityLookup {
  fn new() -> Self {
    Self
  }
}

impl IdentityLookup for PendingIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, _key: &GrainKey, _now: u64) -> Result<PlacementResolution, LookupError> {
    Err(LookupError::Pending)
  }
}

struct RecordingDownProvider {
  downed: ArcShared<NoStdMutex<Vec<String>>>,
}

impl ClusterProvider for RecordingDownProvider {
  fn start_member(&mut self) -> Result<(), crate::core::ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), crate::core::ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, authority: &str) -> Result<(), crate::core::ClusterProviderError> {
    self.downed.lock().push(String::from(authority));
    Ok(())
  }

  fn join(&mut self, _authority: &str) -> Result<(), crate::core::ClusterProviderError> {
    Ok(())
  }

  fn leave(&mut self, _authority: &str) -> Result<(), crate::core::ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), crate::core::ClusterProviderError> {
    Ok(())
  }
}

struct RecordingMembershipProvider {
  joined: ArcShared<NoStdMutex<Vec<String>>>,
  left:   ArcShared<NoStdMutex<Vec<String>>>,
}

impl ClusterProvider for RecordingMembershipProvider {
  fn start_member(&mut self) -> Result<(), crate::core::ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), crate::core::ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), crate::core::ClusterProviderError> {
    Ok(())
  }

  fn join(&mut self, authority: &str) -> Result<(), crate::core::ClusterProviderError> {
    self.joined.lock().push(String::from(authority));
    Ok(())
  }

  fn leave(&mut self, authority: &str) -> Result<(), crate::core::ClusterProviderError> {
    self.left.lock().push(String::from(authority));
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), crate::core::ClusterProviderError> {
    Ok(())
  }
}

struct RecordingDowningProvider {
  downed: ArcShared<NoStdMutex<Vec<String>>>,
}

impl DowningProvider for RecordingDowningProvider {
  fn down(&mut self, authority: &str) -> Result<(), crate::core::ClusterProviderError> {
    self.downed.lock().push(String::from(authority));
    Ok(())
  }
}

struct TestActorRefProvider {
  system: ActorSystemGeneric<NoStdToolbox>,
}

impl TestActorRefProvider {
  fn new(system: ActorSystemGeneric<NoStdToolbox>) -> Self {
    Self { system }
  }
}

impl ActorRefProvider<NoStdToolbox> for TestActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    static SCHEMES: [ActorPathScheme; 1] = [ActorPathScheme::FraktorTcp];
    &SCHEMES
  }

  fn actor_ref(&mut self, _path: ActorPath) -> Result<ActorRefGeneric<NoStdToolbox>, ActorError> {
    let sender = ActorRefSenderSharedGeneric::new(TestSender);
    Ok(ActorRefGeneric::from_shared(Pid::new(1, 0), sender, &self.system.state()))
  }
}

struct TestSender;

impl ActorRefSender<NoStdToolbox> for TestSender {
  fn send(
    &mut self,
    _message: AnyMessageGeneric<NoStdToolbox>,
  ) -> Result<SendOutcome, fraktor_actor_rs::core::error::SendError<NoStdToolbox>> {
    Ok(SendOutcome::Delivered)
  }
}
