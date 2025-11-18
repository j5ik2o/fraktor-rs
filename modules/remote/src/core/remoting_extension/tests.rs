#![cfg(any(test, feature = "test-support"))]

use alloc::{format, vec::Vec};

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric},
  error::ActorError,
  event_stream::{
    event_stream_event::EventStreamEvent,
    remoting_lifecycle_event::RemotingLifecycleEvent,
    EventStreamSubscriber,
    EventStreamSubscriptionGeneric,
    BackpressureSignal,
  },
  extension::ExtensionsConfig,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  system::{ActorSystemBuilder, ActorSystemGeneric},
};
use fraktor_utils_rs::core::{runtime_toolbox::{NoStdMutex, NoStdToolbox}, sync::ArcShared};

use crate::core::{
  remoting_backpressure_listener::FnRemotingBackpressureListener,
  remoting_extension_config::RemotingExtensionConfig,
  remoting_extension_id::RemotingExtensionId,
  remoting_control_handle::RemotingControlHandle,
  remoting_error::RemotingError,
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
struct EventRecorder {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl EventRecorder {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn snapshot(&self) -> Vec<EventStreamEvent<NoStdToolbox>> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber<NoStdToolbox> for EventRecorder {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

fn bootstrap(
  config: RemotingExtensionConfig,
) -> (ActorSystemGeneric<NoStdToolbox>, RemotingControlHandle<NoStdToolbox>) {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("remoting-test-guardian");
  let extensions = ExtensionsConfig::default().with_extension_config(config.clone());
  let system = ActorSystemBuilder::new(props)
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()))
    .with_extensions_config(extensions)
    .build()
    .expect("actor system");
  let id = RemotingExtensionId::<NoStdToolbox>::new(config);
  let extension = system.extended().extension(&id).expect("extension registered");
  (system, extension.handle())
}

fn subscribe_events(
  system: &ActorSystemGeneric<NoStdToolbox>,
) -> (EventRecorder, EventStreamSubscriptionGeneric<NoStdToolbox>) {
  let recorder = EventRecorder::new();
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = ArcShared::new(recorder.clone());
  let subscription = system.subscribe_event_stream(&subscriber);
  (recorder, subscription)
}

fn captured_lifecycle(events: &[EventStreamEvent<NoStdToolbox>]) -> Vec<RemotingLifecycleEvent> {
  events
    .iter()
    .filter_map(|event| match event {
      | EventStreamEvent::RemotingLifecycle(event) => Some(event.clone()),
      | _ => None,
    })
    .collect()
}

#[test]
fn manual_start_publishes_lifecycle_events() {
  let config = RemotingExtensionConfig::default().with_auto_start(false);
  let (system, handle) = bootstrap(config);
  let (recorder, _subscription) = subscribe_events(&system);

  assert!(!handle.is_running());
  handle.start().expect("start succeeds");

  let events = recorder.snapshot();
  let lifecycle = captured_lifecycle(&events);
  assert!(matches!(lifecycle.as_slice(), [RemotingLifecycleEvent::Starting, RemotingLifecycleEvent::Started]));
}

#[test]
fn auto_start_enabled_runs_by_default() {
  let config = RemotingExtensionConfig::default();
  let (_system, handle) = bootstrap(config);
  assert!(handle.is_running());
}

#[test]
fn shutdown_emits_shutdown_event() {
  let config = RemotingExtensionConfig::default().with_auto_start(false);
  let (system, handle) = bootstrap(config);
  let (recorder, _subscription) = subscribe_events(&system);

  handle.start().expect("started");
  recorder.events.lock().clear();

  handle.shutdown().expect("shutdown succeeds");
  let lifecycle = captured_lifecycle(&recorder.snapshot());
  assert!(lifecycle.iter().any(|event| matches!(event, RemotingLifecycleEvent::Shutdown)));

  let second = handle.shutdown();
  assert!(matches!(second, Err(RemotingError::AlreadyShutdown)));
}

#[test]
fn backpressure_listener_invoked_and_eventstream_emits() {
  let config_calls: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  let config = RemotingExtensionConfig::default().with_backpressure_listener({
    let captured = config_calls.clone();
    move |signal, authority, _| captured.lock().push(format!("{authority}:{signal:?}"))
  });
  let (system, handle) = bootstrap(config);
  let runtime_calls: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  handle.register_backpressure_listener(FnRemotingBackpressureListener::new({
    let captured = runtime_calls.clone();
    move |signal, authority, _| captured.lock().push(format!("{authority}:{signal:?}"))
  }));
  let (recorder, _subscription) = subscribe_events(&system);

  handle.emit_backpressure_signal("loopback:9000", BackpressureSignal::Apply);

  let config_snapshot = config_calls.lock().clone();
  assert_eq!(config_snapshot, vec!["loopback:9000:Apply".to_string()]);

  let runtime_snapshot = runtime_calls.lock().clone();
  assert_eq!(runtime_snapshot, vec!["loopback:9000:Apply".to_string()]);

  let emitted = recorder.snapshot();
  assert!(emitted.iter().any(|event| matches!(event, EventStreamEvent::RemotingBackpressure(_))));
}
