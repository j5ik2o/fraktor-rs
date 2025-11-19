#![cfg(feature = "test-support")]

extern crate alloc;

use alloc::{format, vec::Vec};

use anyhow::{Result, anyhow};
use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, Pid, actor_path::ActorPathParts},
  config::ActorSystemConfig,
  error::ActorError,
  event_stream::{
    BackpressureSignal, EventStreamEvent, EventStreamSubscriber, EventStreamSubscriptionGeneric, RemotingLifecycleEvent,
  },
  extension::ExtensionInstallers,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  serialization::SerializationExtensionInstaller,
  system::{ActorSystemGeneric, AuthorityState, RemoteWatchHook},
};
use fraktor_remote_rs::core::{
  FlightMetricKind, FnRemotingBackpressureListener, LoopbackActorRefProvider, LoopbackActorRefProviderInstaller,
  RemotingControl, RemotingControlHandle, RemotingExtensionConfig, RemotingExtensionId, RemotingExtensionInstaller,
  default_loopback_setup,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

struct NoopActor;

impl Actor<NoStdToolbox> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[derive(Clone)]
struct RecordingSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl RecordingSubscriber {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }
}

impl EventStreamSubscriber<NoStdToolbox> for RecordingSubscriber {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

fn build_system(
  config: RemotingExtensionConfig,
) -> (ActorSystemGeneric<NoStdToolbox>, RemotingControlHandle<NoStdToolbox>) {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("quickstart-guardian");
  let serialization_installer = SerializationExtensionInstaller::new(default_loopback_setup());
  let extensions = ExtensionInstallers::default()
    .with_extension_installer(serialization_installer)
    .with_extension_installer(RemotingExtensionInstaller::new(config.clone()));
  let remoting_config = fraktor_actor_rs::core::config::RemotingConfig::default()
    .with_canonical_host("127.0.0.1")
    .with_canonical_port(25500);
  let system_config = ActorSystemConfig::default()
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()))
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(LoopbackActorRefProviderInstaller::default())
    .with_remoting_config(remoting_config);
  let system = ActorSystemGeneric::new_with_config(&props, &system_config).expect("system");
  let id = RemotingExtensionId::<NoStdToolbox>::new(config);
  let extension = system.extended().extension(&id).expect("extension registered");
  (system, extension.handle())
}

fn subscribe(
  system: &ActorSystemGeneric<NoStdToolbox>,
) -> (RecordingSubscriber, EventStreamSubscriptionGeneric<NoStdToolbox>) {
  let subscriber_impl = RecordingSubscriber::new();
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = ArcShared::new(subscriber_impl.clone());
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

#[test]
fn quickstart_loopback_provider_flow() -> Result<()> {
  let config_hits: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  // canonical_host/port は ActorSystemConfig の RemotingConfig から自動的に取得される
  let config = RemotingExtensionConfig::default().with_auto_start(false).with_backpressure_listener({
    let hits = config_hits.clone();
    move |signal, authority, _| hits.lock().push(format!("{authority}:{signal:?}"))
  });
  let (system, handle) = build_system(config);
  let provider = system.extended().actor_ref_provider::<LoopbackActorRefProvider>().expect("provider installed");
  let runtime_hits: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  handle.register_backpressure_listener(FnRemotingBackpressureListener::new({
    let hits = runtime_hits.clone();
    move |signal, authority, _| hits.lock().push(format!("{authority}:{signal:?}"))
  }));
  let (recorder, _subscription) = subscribe(&system);

  assert!(!handle.is_running());
  handle.start().map_err(|error| anyhow!("{error}"))?;

  provider
    .watch_remote(ActorPathParts::with_authority("remote-system", Some(("127.0.0.1", 4321))))
    .map_err(|error| anyhow!("{error}"))?;

  handle.emit_backpressure_signal("127.0.0.1:4321", BackpressureSignal::Apply);

  assert!(matches!(
    recorder
      .events
      .lock()
      .iter()
      .filter_map(|event| match event {
        | EventStreamEvent::RemotingLifecycle(event) => Some(event.clone()),
        | _ => None,
      })
      .collect::<Vec<_>>()
      .as_slice(),
    [RemotingLifecycleEvent::Starting, RemotingLifecycleEvent::Started]
  ));

  let config_snapshot = config_hits.lock().clone();
  assert_eq!(config_snapshot, vec!["127.0.0.1:4321:Apply".to_string()]);
  let runtime_snapshot = runtime_hits.lock().clone();
  assert_eq!(runtime_snapshot, vec!["127.0.0.1:4321:Apply".to_string()]);

  assert!(recorder.events.lock().iter().any(|event| matches!(event, EventStreamEvent::RemotingBackpressure(_))));

  let authority = "127.0.0.1:4321";
  let snapshots = provider.connections_snapshot();
  let snapshot = snapshots.iter().find(|entry| entry.authority() == authority).expect("snapshot exists");
  assert!(matches!(snapshot.state(), AuthorityState::Unresolved));

  let recorder_snapshot = handle.flight_recorder_snapshot();
  let metrics = recorder_snapshot.records();
  assert!(matches!(
    metrics.last(),
    Some(metric)
      if metric.authority() == authority
        && matches!(metric.kind(), FlightMetricKind::Backpressure(BackpressureSignal::Apply))
  ));

  Ok(())
}

#[test]
fn remote_provider_enqueues_message() -> Result<()> {
  let config = RemotingExtensionConfig::default().with_auto_start(false);
  let (system, handle) = build_system(config);
  handle.start().map_err(|error| anyhow!("{error}"))?;
  let provider = system.extended().actor_ref_provider::<LoopbackActorRefProvider>().expect("provider installed");
  provider
    .watch_remote(ActorPathParts::with_authority("remote-system", Some(("127.0.0.1", 25520))))
    .map_err(|error| anyhow!("{error}"))?;
  let remote = provider.actor_ref(remote_path()).expect("actor ref");
  remote.tell(AnyMessageGeneric::new("loopback".to_string())).expect("send succeeds");

  let writer = provider.writer_for_test();
  let envelope = writer.try_next().expect("poll writer").expect("envelope");
  assert_eq!(envelope.recipient().to_relative_string(), "/user/user/svc");
  assert_eq!(envelope.remote_node().host(), "127.0.0.1");
  assert_eq!(envelope.remote_node().port(), Some(25520));
  Ok(())
}

#[test]
fn remote_watch_hook_handles_system_watch_messages() -> Result<()> {
  let config = RemotingExtensionConfig::default().with_auto_start(false);
  let (system, handle) = build_system(config);
  handle.start().map_err(|error| anyhow!("{error}"))?;
  let provider = system.extended().actor_ref_provider::<LoopbackActorRefProvider>().expect("provider installed");
  let remote = provider.actor_ref(remote_path()).expect("remote actor ref");
  let watcher = Pid::new(7777, 0);

  assert!(RemoteWatchHook::handle_watch(&*provider, remote.pid(), watcher));

  let watchers = provider.remote_watchers_for_test(remote.pid()).expect("entry snapshot");
  assert_eq!(watchers, vec![watcher]);
  Ok(())
}
