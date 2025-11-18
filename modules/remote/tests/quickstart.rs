#![cfg(feature = "test-support")]

extern crate alloc;

use alloc::{format, vec::Vec};

use anyhow::{Result, anyhow};
use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric},
  error::ActorError,
  event_stream::{
    BackpressureSignal, EventStreamEvent, EventStreamSubscriber, EventStreamSubscriptionGeneric, RemotingLifecycleEvent,
  },
  extension::ExtensionsConfig,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  system::{ActorSystemBuilder, ActorSystemGeneric},
};
use fraktor_remote_rs::core::{
  FnRemotingBackpressureListener, RemotingControl, RemotingControlHandle, RemotingExtensionConfig, RemotingExtensionId,
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
  let extensions = ExtensionsConfig::default().with_extension_config(config.clone());
  let system = ActorSystemBuilder::new(props)
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()))
    .with_extensions_config(extensions)
    .build()
    .expect("system");
  let id = RemotingExtensionId::<NoStdToolbox>::new(config);
  let extension = system.extension(&id).expect("extension registered");
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

#[test]
fn quickstart_loopback_provider_flow() -> Result<()> {
  let config_hits: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  let config = RemotingExtensionConfig::default()
    .with_canonical_host("127.0.0.1")
    .with_canonical_port(4321)
    .with_auto_start(false)
    .with_backpressure_listener({
      let hits = config_hits.clone();
      move |signal, authority, _| hits.lock().push(format!("{authority}:{signal:?}"))
    });
  let (system, handle) = build_system(config);
  let runtime_hits: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  handle.register_backpressure_listener(FnRemotingBackpressureListener::new({
    let hits = runtime_hits.clone();
    move |signal, authority, _| hits.lock().push(format!("{authority}:{signal:?}"))
  }));
  let (recorder, _subscription) = subscribe(&system);

  assert!(!handle.is_running());
  handle.start().map_err(|error| anyhow!("{error}"))?;

  handle.emit_backpressure_for_test("127.0.0.1:4321", BackpressureSignal::Apply);

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

  Ok(())
}
