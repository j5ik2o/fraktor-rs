use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, Pid,
    actor_path::{ActorPath, ActorPathScheme, PathSegment},
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    actor_ref_provider::{ActorRefProvider, ActorRefProviderShared},
    error::{ActorError, SendError},
    extension::ExtensionInstallers,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::{
      SchedulerConfig, SchedulerShared,
      tick_driver::{ManualTestDriver, TickDriverConfig},
    },
    setup::ActorSystemConfig,
  },
  event::stream::{
    EventStreamEvent, EventStreamShared, EventStreamSubscriber, EventStreamSubscriberShared, EventStreamSubscription,
    subscriber_handle_with_lock_provider,
  },
  system::{
    ActorSystem, TerminationSignal,
    lock_provider::{ActorLockProvider, BuiltinSpinLockProvider},
  },
};
use fraktor_utils_core_rs::core::{
  sync::{ArcShared, SharedAccess, SpinSyncMutex},
  time::TimerInstant,
};

use crate::core::{
  ClusterApi, ClusterExtension, ClusterExtensionConfig, ClusterExtensionInstaller,
  cluster_provider::NoopClusterProvider,
  grain::{GRAIN_EVENT_STREAM_NAME, GrainCallOptions, GrainEvent, GrainKey, GrainRef, GrainRetryPolicy},
  identity::{ClusterIdentity, IdentityLookup, IdentitySetupError, LookupError},
  placement::{ActivatedKind, PlacementDecision, PlacementLocality, PlacementResolution},
};

#[test]
fn grain_ref_get_returns_resolved_ref_with_identity() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")), None);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let grain_ref = GrainRef::new(api, identity.clone());

  let resolved = grain_ref.get().expect("resolved");
  assert_eq!(resolved.identity, identity);
  assert_eq!(resolved.actor_ref.pid(), Pid::new(1, 0));
}

#[test]
fn request_retries_on_timeout_until_policy_exhausted() {
  let send_counter = ArcShared::new(SpinSyncMutex::new(0usize));
  let send_counter_ref = Some(&send_counter);
  let (system, ext) = build_system_with_extension_config(
    || Box::new(StaticIdentityLookup::new("node1:8080")),
    send_counter_ref,
    true,
    SendBehavior::Ok,
  );
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let event_stream = system.event_stream();
  let (recorder, _subscription) = subscribe_grain_events(&event_stream);

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let options = GrainCallOptions::new(Some(Duration::from_millis(1)), GrainRetryPolicy::Fixed {
    max_retries: 2,
    delay:       Duration::from_millis(1),
  });
  let grain_ref = GrainRef::new(api, identity).with_options(options);

  let response = grain_ref.request(&AnyMessage::new(())).expect("request");

  run_scheduler(&system, Duration::from_millis(10));

  let result = response.future().with_write(|inner| inner.try_take()).expect("timeout payload");
  assert!(result.is_err(), "expect timeout error");
  let ask_error = result.unwrap_err();
  assert_eq!(ask_error, fraktor_actor_core_rs::core::kernel::actor::messaging::AskError::Timeout);

  let sends = *send_counter.lock();
  assert_eq!(sends, 3);

  let events = recorder.events();
  let retry_events = events.iter().filter(|e| matches!(e, GrainEvent::CallRetrying { .. })).count();
  assert_eq!(retry_events, 2);
  assert!(events.iter().any(|e| matches!(e, GrainEvent::CallTimedOut { .. })));

  let metrics = ext.grain_metrics().expect("metrics");
  assert_eq!(metrics.call_retries(), 2);
  assert_eq!(metrics.call_timeouts(), 1);
}

#[test]
fn request_emits_failure_event_and_updates_metrics() {
  let (system, ext) = build_system_with_extension_config(
    || Box::new(StaticIdentityLookup::new("node1:8080")),
    None,
    true,
    SendBehavior::Fail,
  );
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let event_stream = system.event_stream();
  let (recorder, _subscription) = subscribe_grain_events(&event_stream);

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let grain_ref = GrainRef::new(api, identity.clone());

  let err = match grain_ref.request(&AnyMessage::new(())) {
    | Ok(_) => panic!("request should fail"),
    | Err(err) => err,
  };
  assert!(matches!(err, crate::core::grain::GrainCallError::RequestFailed(_)));

  let events = recorder.events();
  assert!(events.iter().any(|event| matches!(event, GrainEvent::CallFailed { identity: id, .. } if id == &identity)));

  let metrics = ext.grain_metrics().expect("metrics");
  assert_eq!(metrics.call_failures(), 1);
}

fn run_scheduler(system: &ActorSystem, duration: Duration) {
  let scheduler: SchedulerShared = system.state().scheduler();
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
  send_counter: Option<&ArcShared<SpinSyncMutex<usize>>>,
) -> (ActorSystem, ArcShared<ClusterExtension>)
where
  F: Fn() -> Box<dyn IdentityLookup> + Send + Sync + 'static, {
  build_system_with_extension_config(identity_lookup_factory, send_counter, false, SendBehavior::Ok)
}

fn build_system_with_extension_config<F>(
  identity_lookup_factory: F,
  send_counter: Option<&ArcShared<SpinSyncMutex<usize>>>,
  metrics_enabled: bool,
  send_behavior: SendBehavior,
) -> (ActorSystem, ArcShared<ClusterExtension>)
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
  let send_counter = send_counter.cloned();
  let config = ActorSystemConfig::default()
    .with_scheduler_config(scheduler_config)
    .with_tick_driver(tick_driver)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(move |system: &ActorSystem| {
      let provider =
        ActorRefProviderShared::new(TestActorRefProvider::new(system.clone(), send_counter.clone(), send_behavior));
      system.extended().register_actor_ref_provider(&provider)
    });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::new_with_config(&props, &config).expect("build system");
  let extension = system.extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  (system, extension)
}

#[derive(Clone, Copy)]
enum SendBehavior {
  Ok,
  Fail,
  Reply,
}

#[derive(Clone)]
struct RecordingGrainEvents {
  events: ArcShared<SpinSyncMutex<Vec<GrainEvent>>>,
}

impl RecordingGrainEvents {
  fn new() -> Self {
    Self { events: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<GrainEvent> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber for RecordingGrainEvents {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == GRAIN_EVENT_STREAM_NAME
      && let Some(grain_event) = payload.payload().downcast_ref::<GrainEvent>()
    {
      self.events.lock().push(grain_event.clone());
    }
  }
}

fn subscribe_grain_events(event_stream: &EventStreamShared) -> (RecordingGrainEvents, EventStreamSubscription) {
  let recorder = RecordingGrainEvents::new();
  let subscriber = test_subscriber_handle(recorder.clone());
  let subscription = event_stream.subscribe(&subscriber);
  (recorder, subscription)
}

fn test_subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  let lock_provider: ArcShared<dyn ActorLockProvider> = ArcShared::new(BuiltinSpinLockProvider::new());
  subscriber_handle_with_lock_provider(&lock_provider, subscriber)
}

struct TestGuardian;

impl Actor for TestGuardian {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
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

struct TestActorRefProvider {
  system:   ActorSystem,
  counter:  Option<ArcShared<SpinSyncMutex<usize>>>,
  behavior: SendBehavior,
}

impl TestActorRefProvider {
  fn new(system: ActorSystem, counter: Option<ArcShared<SpinSyncMutex<usize>>>, behavior: SendBehavior) -> Self {
    Self { system, counter, behavior }
  }
}

impl ActorRefProvider for TestActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    static SCHEMES: [ActorPathScheme; 1] = [ActorPathScheme::FraktorTcp];
    &SCHEMES
  }

  fn actor_ref(&mut self, _path: ActorPath) -> Result<ActorRef, ActorError> {
    Ok(ActorRef::with_system(
      Pid::new(1, 0),
      TestSender { counter: self.counter.clone(), behavior: self.behavior },
      &self.system.state(),
    ))
  }

  fn termination_signal(&self) -> TerminationSignal {
    TerminationSignal::already_terminated()
  }
}

struct TestSender {
  counter:  Option<ArcShared<SpinSyncMutex<usize>>>,
  behavior: SendBehavior,
}

impl ActorRefSender for TestSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if matches!(self.behavior, SendBehavior::Fail) {
      return Err(SendError::timeout(AnyMessage::new(())));
    }
    if matches!(self.behavior, SendBehavior::Reply)
      && let Some(mut sender) = message.sender().cloned()
    {
      let reply = AnyMessage::new(String::from("reply"));
      sender.tell(reply);
    }
    if let Some(counter) = &self.counter {
      *counter.lock() += 1;
    }
    Ok(SendOutcome::Delivered)
  }
}

#[test]
fn request_with_sender_forwards_reply_and_completes_future() {
  let (system, ext) = build_system_with_extension_config(
    || Box::new(StaticIdentityLookup::new("node1:8080")),
    None,
    false,
    SendBehavior::Reply,
  );
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let grain_ref = GrainRef::new(api, identity);

  let recorder = RecordingSender::new();
  let sender_ref = ActorRef::with_system(Pid::new(99, 0), recorder.clone(), &system.state());

  let response =
    grain_ref.request_with_sender(&AnyMessage::new(String::from("ping")), &sender_ref).expect("request with sender");

  let result = response.future().with_write(|inner| inner.try_take()).expect("future ready");
  let reply = result.expect("reply ok");
  let reply_text = reply.payload().downcast_ref::<String>().expect("reply string");
  assert_eq!(reply_text, "reply");

  let forwarded = recorder.messages();
  assert_eq!(forwarded.len(), 1);
  let forwarded_text = forwarded[0].payload().downcast_ref::<String>().expect("forwarded string");
  assert_eq!(forwarded_text, "reply");
}

#[test]
fn request_with_sender_forward_failure_completes_error_and_emits_event() {
  let (system, ext) = build_system_with_extension_config(
    || Box::new(StaticIdentityLookup::new("node1:8080")),
    None,
    true,
    SendBehavior::Reply,
  );
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let event_stream = system.event_stream();
  let (recorder, _subscription) = subscribe_grain_events(&event_stream);

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let grain_ref = GrainRef::new(api, identity.clone());

  let sender_ref = ActorRef::with_system(Pid::new(98, 0), FailingSender, &system.state());
  let response =
    grain_ref.request_with_sender(&AnyMessage::new(String::from("ping")), &sender_ref).expect("request with sender");

  let result = response.future().with_write(|inner| inner.try_take()).expect("future ready");
  let ask_error = result.expect_err("expect send failed");
  assert!(matches!(ask_error, fraktor_actor_core_rs::core::kernel::actor::messaging::AskError::SendFailed(_)));

  let events = recorder.events();
  assert!(events.iter().any(|event| matches!(event, GrainEvent::CallFailed { identity: id, .. } if id == &identity)));

  let metrics = ext.grain_metrics().expect("metrics");
  assert_eq!(metrics.call_failures(), 1);
}

#[test]
fn request_with_sender_cleans_temp_actor_on_completion() {
  let (system, ext) = build_system_with_extension_config(
    || Box::new(StaticIdentityLookup::new("node1:8080")),
    None,
    false,
    SendBehavior::Reply,
  );
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let grain_ref = GrainRef::new(api, identity);

  let recorder = RecordingSender::new();
  let sender_ref = ActorRef::with_system(Pid::new(97, 0), recorder, &system.state());
  let response =
    grain_ref.request_with_sender(&AnyMessage::new(String::from("ping")), &sender_ref).expect("request with sender");

  let _ = response.future().with_write(|inner| inner.try_take());
  if let Some(temp_path) = response.sender().path() {
    let temp_name = temp_path.segments().last().map(PathSegment::as_str).expect("temp name").to_string();
    assert!(system.state().temp_actor(&temp_name).is_none(), "temp actor should be cleaned");
  }
}

#[derive(Clone)]
struct RecordingSender {
  messages: ArcShared<SpinSyncMutex<Vec<AnyMessage>>>,
}

impl RecordingSender {
  fn new() -> Self {
    Self { messages: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn messages(&self) -> Vec<AnyMessage> {
    self.messages.lock().clone()
  }
}

impl ActorRefSender for RecordingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

struct FailingSender;

impl ActorRefSender for FailingSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::timeout(AnyMessage::new(())))
  }
}
