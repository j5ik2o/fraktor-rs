#![cfg(feature = "test-support")]

extern crate alloc;

use alloc::{format, vec::Vec};

use anyhow::{Result, anyhow};
use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, Pid, actor_path::ActorPathParts},
  error::ActorError,
  event_stream::{
    BackpressureSignal, EventStreamEvent, EventStreamSubscriber, EventStreamSubscriptionGeneric,
    RemotingLifecycleEvent, subscriber_handle,
  },
  extension::ExtensionInstallers,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  serialization::SerializationExtensionInstaller,
  system::{
    ActorRefProvider, ActorSystemConfigGeneric, ActorSystemGeneric, AuthorityState, RemoteWatchHook,
    RemoteWatchHookShared, RemotingConfig,
  },
};
use fraktor_remote_rs::core::{
  FlightMetricKind, FnRemotingBackpressureListener, LoopbackActorRefProviderGeneric, LoopbackActorRefProviderInstaller,
  RemotingControl, RemotingControlShared, RemotingExtensionConfig, RemotingExtensionId, RemotingExtensionInstaller,
  default_loopback_setup,
};
use fraktor_utils_rs::{
  core::{runtime_toolbox::NoStdMutex, sync::ArcShared},
  std::runtime_toolbox::StdToolbox,
};

struct NoopActor;

impl Actor<StdToolbox> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, StdToolbox>,
    _message: AnyMessageViewGeneric<'_, StdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[derive(Clone)]
struct RecordingSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<StdToolbox>>>>,
}

impl RecordingSubscriber {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }
}

impl EventStreamSubscriber<StdToolbox> for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent<StdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

fn build_system(
  config: RemotingExtensionConfig,
) -> (ActorSystemGeneric<StdToolbox>, RemotingControlShared<StdToolbox>) {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("quickstart-guardian");
  let serialization_installer = SerializationExtensionInstaller::new(default_loopback_setup());
  let extensions = ExtensionInstallers::<StdToolbox>::default()
    .with_extension_installer(serialization_installer)
    .with_extension_installer(RemotingExtensionInstaller::new(config.clone()));
  let remoting_config = RemotingConfig::default().with_canonical_host("127.0.0.1").with_canonical_port(25500);
  let system_config = ActorSystemConfigGeneric::<StdToolbox>::default()
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::<StdToolbox>::new()))
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(LoopbackActorRefProviderInstaller::default())
    .with_remoting_config(remoting_config);
  let system = ActorSystemGeneric::new_with_config(&props, &system_config).expect("system");
  let id = RemotingExtensionId::new(config);
  let extension = system.extended().extension(&id).expect("extension registered");
  (system, extension.handle())
}

fn subscribe(
  system: &ActorSystemGeneric<StdToolbox>,
) -> (RecordingSubscriber, EventStreamSubscriptionGeneric<StdToolbox>) {
  let subscriber_impl = RecordingSubscriber::new();
  let subscriber = subscriber_handle(subscriber_impl.clone());
  let subscription = system.subscribe_event_stream(&subscriber);
  (subscriber_impl, subscription)
}

fn remote_path() -> fraktor_actor_rs::core::actor_prim::actor_path::ActorPath {
  use fraktor_actor_rs::core::actor_prim::actor_path::{ActorPath, ActorPathParts, GuardianKind};
  let mut parts = ActorPathParts::with_authority("remote-system", Some(("127.0.0.1", 25520)));
  parts = parts.with_guardian(GuardianKind::User);
  let mut path = ActorPath::from_parts(parts);
  path = path.child("user");
  path = path.child("svc");
  path
}

#[tokio::test]
async fn quickstart_loopback_provider_flow() -> Result<()> {
  type SharedProvider = RemoteWatchHookShared<StdToolbox, LoopbackActorRefProviderGeneric<StdToolbox>>;
  let config_hits: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  // canonical_host/port は ActorSystemConfig の RemotingConfig から自動的に取得される
  let config = RemotingExtensionConfig::default().with_auto_start(false).with_backpressure_listener({
    let hits = config_hits.clone();
    move |signal, authority, _| hits.lock().push(format!("{authority}:{signal:?}"))
  });
  let (system, handle) = build_system(config);
  let provider = system.extended().actor_ref_provider::<SharedProvider>().expect("provider installed");
  let runtime_hits: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  handle.lock().register_backpressure_listener(FnRemotingBackpressureListener::new({
    let hits = runtime_hits.clone();
    move |signal, authority, _| hits.lock().push(format!("{authority}:{signal:?}"))
  }));
  let (recorder, _subscription) = subscribe(&system);

  assert!(!handle.lock().is_running());
  handle.lock().start().map_err(|error| anyhow!("{error}"))?;

  // Wait for async startup to complete
  tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

  provider
    .inner()
    .lock()
    .watch_remote(ActorPathParts::with_authority("remote-system", Some(("127.0.0.1", 4321))))
    .map_err(|error| anyhow!("{error}"))?;

  handle.lock().emit_backpressure_signal("127.0.0.1:4321", BackpressureSignal::Apply);

  // Wait for events to propagate
  tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

  let lifecycle_events = recorder
    .events
    .lock()
    .iter()
    .filter_map(|event| match event {
      | EventStreamEvent::RemotingLifecycle(event) => Some(event.clone()),
      | _ => None,
    })
    .collect::<Vec<_>>();

  assert!(
    lifecycle_events.len() >= 2
      && matches!(&lifecycle_events[0], RemotingLifecycleEvent::Starting)
      && matches!(&lifecycle_events[1], RemotingLifecycleEvent::Started),
    "Expected at least [Starting, Started], got: {:?}",
    lifecycle_events
  );

  let config_snapshot = config_hits.lock().clone();
  assert_eq!(config_snapshot, vec!["127.0.0.1:4321:Apply".to_string()]);
  let runtime_snapshot = runtime_hits.lock().clone();
  assert_eq!(runtime_snapshot, vec!["127.0.0.1:4321:Apply".to_string()]);

  assert!(recorder.events.lock().iter().any(|event| matches!(event, EventStreamEvent::RemotingBackpressure(_))));

  let authority = "127.0.0.1:4321";
  let snapshots = provider.inner().lock().connections_snapshot();
  let snapshot = snapshots.iter().find(|entry| entry.authority() == authority).expect("snapshot exists");
  assert!(matches!(snapshot.state(), AuthorityState::Unresolved));

  let recorder_snapshot = handle.lock().flight_recorder_snapshot();
  let metrics = recorder_snapshot.records();
  assert!(matches!(
    metrics.last(),
    Some(metric)
      if metric.authority() == authority
        && matches!(metric.kind(), FlightMetricKind::Backpressure(BackpressureSignal::Apply))
  ));

  Ok(())
}

#[tokio::test]
async fn remote_provider_enqueues_message() -> Result<()> {
  type SharedProvider = RemoteWatchHookShared<StdToolbox, LoopbackActorRefProviderGeneric<StdToolbox>>;
  let config = RemotingExtensionConfig::default().with_auto_start(false);
  let (system, handle) = build_system(config);
  handle.lock().start().map_err(|error| anyhow!("{error}"))?;
  let provider = system.extended().actor_ref_provider::<SharedProvider>().expect("provider installed");
  provider
    .inner()
    .lock()
    .watch_remote(ActorPathParts::with_authority("remote-system", Some(("127.0.0.1", 25520))))
    .map_err(|error| anyhow!("{error}"))?;
  let remote = provider.actor_ref(remote_path()).expect("actor ref");
  remote.tell(AnyMessageGeneric::new("loopback".to_string())).expect("send succeeds");

  let writer = provider.inner().lock().writer_for_test();
  let envelope = writer.lock().try_next().expect("poll writer").expect("envelope");
  assert_eq!(envelope.recipient().to_relative_string(), "/user/user/svc");
  assert_eq!(envelope.remote_node().host(), "127.0.0.1");
  assert_eq!(envelope.remote_node().port(), Some(25520));
  Ok(())
}

#[tokio::test]
async fn remote_watch_hook_handles_system_watch_messages() -> Result<()> {
  let config = RemotingExtensionConfig::default().with_auto_start(false);
  let (system, handle) = build_system(config);
  handle.lock().start().map_err(|error| anyhow!("{error}"))?;
  type SharedProvider = RemoteWatchHookShared<StdToolbox, LoopbackActorRefProviderGeneric<StdToolbox>>;
  let provider = system.extended().actor_ref_provider::<SharedProvider>().expect("provider installed");
  let remote = provider.actor_ref(remote_path()).expect("remote actor ref");
  let watcher = Pid::new(7777, 0);

  let mut shared_clone = (*provider).clone();
  assert!(RemoteWatchHook::handle_watch(&mut shared_clone, remote.pid(), watcher));

  let watchers = provider.inner().lock().remote_watchers_for_test(remote.pid()).expect("entry snapshot");
  assert_eq!(watchers, vec![watcher]);
  Ok(())
}
