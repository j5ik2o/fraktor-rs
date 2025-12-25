use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_actor_rs::core::{
  actor_prim::{
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
  scheduler::{ManualTestDriver, SchedulerConfig, SchedulerSharedGeneric, TickDriverConfig},
  system::{ActorRefProvider, ActorRefProviderSharedGeneric, ActorSystemConfigGeneric, ActorSystemGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::{ArcShared, SharedAccess},
  time::TimerInstant,
};

use crate::core::{
  ActivatedKind, ClusterApiError, ClusterApiGeneric, ClusterExtensionConfig, ClusterExtensionGeneric,
  ClusterExtensionInstaller, ClusterIdentity, ClusterRequestError, ClusterResolveError, GRAIN_EVENT_STREAM_NAME,
  GrainEvent, GrainKey, IdentityLookup, IdentitySetupError, LookupError, MetricsError, NoopClusterProvider,
  NoopIdentityLookup, PlacementDecision, PlacementEvent, PlacementLocality, PlacementResolution,
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

struct TestGuardian;

impl Actor<NoStdToolbox> for TestGuardian {
  fn receive(
    &mut self,
    _context: &mut fraktor_actor_rs::core::actor_prim::ActorContextGeneric<'_, NoStdToolbox>,
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
