use alloc::{string::String, vec::Vec};

use fraktor_actor_rs::core::{
  actor::{
    Actor, Pid,
    actor_path::{ActorPath, ActorPathScheme, PathSegment},
    actor_ref::{ActorRefGeneric, ActorRefSender, ActorRefSenderSharedGeneric, SendOutcome},
  },
  error::{ActorError, SendError},
  event::stream::{
    EventStreamEvent, EventStreamSharedGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric,
    subscriber_handle,
  },
  extension::ExtensionInstallers,
  messaging::{AnyMessageGeneric, AskError},
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
  ActivatedKind, ClusterApiGeneric, ClusterExtensionConfig, ClusterExtensionGeneric, ClusterExtensionInstaller,
  ClusterIdentity, GRAIN_EVENT_STREAM_NAME, GrainCallOptions, GrainEvent, GrainKey, GrainRefGeneric, GrainRetryPolicy,
  IdentityLookup, IdentitySetupError, LookupError, NoopClusterProvider, PlacementDecision, PlacementLocality,
  PlacementResolution,
};

#[test]
fn grain_ref_get_returns_resolved_ref_with_identity() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")), None);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let grain_ref = GrainRefGeneric::new(api, identity.clone());

  let resolved = grain_ref.get().expect("resolved");
  assert_eq!(resolved.identity, identity);
  assert_eq!(resolved.actor_ref.pid(), Pid::new(1, 0));
}

#[test]
fn request_retries_on_timeout_until_policy_exhausted() {
  let send_counter = ArcShared::new(NoStdMutex::new(0usize));
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

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let options = GrainCallOptions::new(Some(core::time::Duration::from_millis(1)), GrainRetryPolicy::Fixed {
    max_retries: 2,
    delay:       core::time::Duration::from_millis(1),
  });
  let grain_ref = GrainRefGeneric::new(api, identity).with_options(options);

  let response = grain_ref.request(&AnyMessageGeneric::new(())).expect("request");

  run_scheduler(&system, core::time::Duration::from_millis(10));

  let result = response.future().with_write(|inner| inner.try_take()).expect("timeout payload");
  assert!(result.is_err(), "expect timeout error");
  let ask_error = result.unwrap_err();
  assert_eq!(ask_error, fraktor_actor_rs::core::messaging::AskError::Timeout);

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

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let grain_ref = GrainRefGeneric::new(api, identity.clone());

  let err = match grain_ref.request(&AnyMessageGeneric::new(())) {
    | Ok(_) => panic!("request should fail"),
    | Err(err) => err,
  };
  assert!(matches!(err, crate::core::GrainCallError::RequestFailed(_)));

  let events = recorder.events();
  assert!(events.iter().any(|event| matches!(event, GrainEvent::CallFailed { identity: id, .. } if id == &identity)));

  let metrics = ext.grain_metrics().expect("metrics");
  assert_eq!(metrics.call_failures(), 1);
}

fn run_scheduler(system: &ActorSystemGeneric<NoStdToolbox>, duration: core::time::Duration) {
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
  send_counter: Option<&ArcShared<NoStdMutex<usize>>>,
) -> (ActorSystemGeneric<NoStdToolbox>, ArcShared<ClusterExtensionGeneric<NoStdToolbox>>)
where
  F: Fn() -> Box<dyn IdentityLookup> + Send + Sync + 'static, {
  build_system_with_extension_config(identity_lookup_factory, send_counter, false, SendBehavior::Ok)
}

fn build_system_with_extension_config<F>(
  identity_lookup_factory: F,
  send_counter: Option<&ArcShared<NoStdMutex<usize>>>,
  metrics_enabled: bool,
  send_behavior: SendBehavior,
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
  let send_counter = send_counter.cloned();
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler_config)
    .with_tick_driver(tick_driver)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(move |system: &ActorSystemGeneric<NoStdToolbox>| {
      let provider = ActorRefProviderSharedGeneric::new(TestActorRefProvider::new(
        system.clone(),
        send_counter.clone(),
        send_behavior,
      ));
      system.extended().register_actor_ref_provider(&provider)
    });
  let props = PropsGeneric::from_fn(|| TestGuardian);
  let system = ActorSystemGeneric::new_with_config(&props, &config).expect("build system");
  let extension =
    system.extended().extension_by_type::<ClusterExtensionGeneric<NoStdToolbox>>().expect("cluster extension");
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
    _context: &mut fraktor_actor_rs::core::actor::ActorContextGeneric<'_, NoStdToolbox>,
    _message: fraktor_actor_rs::core::messaging::AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
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
  system:   ActorSystemGeneric<NoStdToolbox>,
  counter:  Option<ArcShared<NoStdMutex<usize>>>,
  behavior: SendBehavior,
}

impl TestActorRefProvider {
  fn new(
    system: ActorSystemGeneric<NoStdToolbox>,
    counter: Option<ArcShared<NoStdMutex<usize>>>,
    behavior: SendBehavior,
  ) -> Self {
    Self { system, counter, behavior }
  }
}

impl ActorRefProvider<NoStdToolbox> for TestActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    static SCHEMES: [ActorPathScheme; 1] = [ActorPathScheme::FraktorTcp];
    &SCHEMES
  }

  fn actor_ref(&mut self, _path: ActorPath) -> Result<ActorRefGeneric<NoStdToolbox>, ActorError> {
    let sender =
      ActorRefSenderSharedGeneric::new(TestSender { counter: self.counter.clone(), behavior: self.behavior });
    Ok(ActorRefGeneric::from_shared(Pid::new(1, 0), sender, &self.system.state()))
  }
}

struct TestSender {
  counter:  Option<ArcShared<NoStdMutex<usize>>>,
  behavior: SendBehavior,
}

impl ActorRefSender<NoStdToolbox> for TestSender {
  fn send(
    &mut self,
    message: AnyMessageGeneric<NoStdToolbox>,
  ) -> Result<SendOutcome, fraktor_actor_rs::core::error::SendError<NoStdToolbox>> {
    if matches!(self.behavior, SendBehavior::Fail) {
      return Err(SendError::timeout(AnyMessageGeneric::new(())));
    }
    if matches!(self.behavior, SendBehavior::Reply)
      && let Some(sender) = message.sender().cloned()
    {
      let reply = AnyMessageGeneric::new(String::from("reply"));
      let _ = sender.tell(reply);
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

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let grain_ref = GrainRefGeneric::new(api, identity);

  let recorder = RecordingSender::new();
  let sender_ref = ActorRefGeneric::new(Pid::new(99, 0), recorder.clone());

  let response = grain_ref
    .request_with_sender(&AnyMessageGeneric::new(String::from("ping")), &sender_ref)
    .expect("request with sender");

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

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let grain_ref = GrainRefGeneric::new(api, identity.clone());

  let sender_ref = ActorRefGeneric::new(Pid::new(98, 0), FailingSender);
  let response = grain_ref
    .request_with_sender(&AnyMessageGeneric::new(String::from("ping")), &sender_ref)
    .expect("request with sender");

  let result = response.future().with_write(|inner| inner.try_take()).expect("future ready");
  let ask_error = result.expect_err("expect send failed");
  assert_eq!(ask_error, AskError::SendFailed);

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

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let grain_ref = GrainRefGeneric::new(api, identity);

  let recorder = RecordingSender::new();
  let sender_ref = ActorRefGeneric::new(Pid::new(97, 0), recorder);
  let response = grain_ref
    .request_with_sender(&AnyMessageGeneric::new(String::from("ping")), &sender_ref)
    .expect("request with sender");

  let _ = response.future().with_write(|inner| inner.try_take());
  if let Some(temp_path) = response.sender().path() {
    let temp_name = temp_path.segments().last().map(PathSegment::as_str).expect("temp name").to_string();
    assert!(system.state().temp_actor(&temp_name).is_none(), "temp actor should be cleaned");
  }
}

#[derive(Clone)]
struct RecordingSender {
  messages: ArcShared<NoStdMutex<Vec<AnyMessageGeneric<NoStdToolbox>>>>,
}

impl RecordingSender {
  fn new() -> Self {
    Self { messages: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn messages(&self) -> Vec<AnyMessageGeneric<NoStdToolbox>> {
    self.messages.lock().clone()
  }
}

impl ActorRefSender<NoStdToolbox> for RecordingSender {
  fn send(
    &mut self,
    message: AnyMessageGeneric<NoStdToolbox>,
  ) -> Result<SendOutcome, fraktor_actor_rs::core::error::SendError<NoStdToolbox>> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

struct FailingSender;

impl ActorRefSender<NoStdToolbox> for FailingSender {
  fn send(
    &mut self,
    _message: AnyMessageGeneric<NoStdToolbox>,
  ) -> Result<SendOutcome, fraktor_actor_rs::core::error::SendError<NoStdToolbox>> {
    Err(SendError::timeout(AnyMessageGeneric::new(())))
  }
}
