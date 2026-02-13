#![cfg(any(test, feature = "test-support"))]

use alloc::{format, sync::Arc, vec::Vec};
use std::sync::Mutex;

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric},
  error::ActorError,
  event::stream::{
    BackpressureSignal, EventStreamEvent, EventStreamSubscriber, EventStreamSubscriptionGeneric,
    RemotingLifecycleEvent, subscriber_handle,
  },
  extension::ExtensionInstallers,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
  system::{ActorSystemConfigGeneric, ActorSystemGeneric},
};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use super::{RemotingControl, RemotingControlShared, RemotingError, RemotingExtensionConfig};
use crate::{
  core::backpressure::FnRemotingBackpressureListener,
  std::{RemotingExtensionId, RemotingExtensionInstaller},
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
struct EventRecorder {
  events: Arc<Mutex<Vec<EventStreamEvent<StdToolbox>>>>,
}

impl EventRecorder {
  fn new() -> Self {
    Self { events: Arc::new(Mutex::new(Vec::new())) }
  }

  fn snapshot(&self) -> Vec<EventStreamEvent<StdToolbox>> {
    self.events.lock().unwrap().clone()
  }
}

impl EventStreamSubscriber<StdToolbox> for EventRecorder {
  fn on_event(&mut self, event: &EventStreamEvent<StdToolbox>) {
    self.events.lock().unwrap().push(event.clone());
  }
}

fn bootstrap(config: RemotingExtensionConfig) -> (ActorSystemGeneric<StdToolbox>, RemotingControlShared<StdToolbox>) {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("remoting-test-guardian");
  let installer = RemotingExtensionInstaller::new(config.clone());
  let extensions = ExtensionInstallers::default().with_extension_installer(installer);
  let system_config = ActorSystemConfigGeneric::<StdToolbox>::default()
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::<StdToolbox>::new()))
    .with_extension_installers(extensions);
  let system = ActorSystemGeneric::new_with_config(&props, &system_config).expect("actor system");
  let id = RemotingExtensionId::new(config);
  let extension = system.extended().extension(&id).expect("extension registered");
  (system, extension.handle())
}

fn subscribe_events(
  system: &ActorSystemGeneric<StdToolbox>,
) -> (EventRecorder, EventStreamSubscriptionGeneric<StdToolbox>) {
  let recorder = EventRecorder::new();
  let subscriber = subscriber_handle(recorder.clone());
  let subscription = system.subscribe_event_stream(&subscriber);
  (recorder, subscription)
}

fn captured_lifecycle(events: &[EventStreamEvent<StdToolbox>]) -> Vec<RemotingLifecycleEvent> {
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

  assert!(!handle.lock().is_running());
  handle.lock().start().expect("start succeeds");

  let events = recorder.snapshot();
  let lifecycle = captured_lifecycle(&events);
  assert!(matches!(lifecycle.as_slice(), [RemotingLifecycleEvent::Starting, RemotingLifecycleEvent::Started]));
}

#[test]
fn auto_start_enabled_runs_by_default() {
  let config = RemotingExtensionConfig::default();
  let (_system, handle) = bootstrap(config);
  assert!(handle.lock().is_running());
}

#[test]
fn shutdown_emits_shutdown_event() {
  let config = RemotingExtensionConfig::default().with_auto_start(false);
  let (system, handle) = bootstrap(config);
  let (recorder, _subscription) = subscribe_events(&system);

  handle.lock().start().expect("started");
  recorder.events.lock().unwrap().clear();

  handle.lock().shutdown().expect("shutdown succeeds");
  let lifecycle = captured_lifecycle(&recorder.snapshot());
  assert!(lifecycle.iter().any(|event| matches!(event, RemotingLifecycleEvent::Shutdown)));

  let second = handle.lock().shutdown();
  assert!(matches!(second, Err(RemotingError::AlreadyShutdown)));
}

#[test]
fn backpressure_listener_invoked_and_eventstream_emits() {
  let config_calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
  let config = RemotingExtensionConfig::default().with_backpressure_listener({
    let captured = config_calls.clone();
    move |signal, authority, _| captured.lock().unwrap().push(format!("{authority}:{signal:?}"))
  });
  let (system, handle) = bootstrap(config);
  let backpressure_calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
  handle.lock().register_backpressure_listener(FnRemotingBackpressureListener::new({
    let captured = backpressure_calls.clone();
    move |signal, authority, _| captured.lock().unwrap().push(format!("{authority}:{signal:?}"))
  }));
  let (recorder, _subscription) = subscribe_events(&system);

  handle.lock().emit_backpressure_signal("loopback:9000", BackpressureSignal::Apply);

  let config_snapshot = config_calls.lock().unwrap().clone();
  assert_eq!(config_snapshot, vec!["loopback:9000:Apply".to_string()]);

  let backpressure_snapshot = backpressure_calls.lock().unwrap().clone();
  assert_eq!(backpressure_snapshot, vec!["loopback:9000:Apply".to_string()]);

  let emitted = recorder.snapshot();
  assert!(emitted.iter().any(|event| matches!(event, EventStreamEvent::RemotingBackpressure(_))));
}
