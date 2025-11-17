use alloc::vec::Vec;

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric},
  error::ActorError,
  event_stream::{
    EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric,
    RemotingLifecycleEvent,
  },
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  system::{ActorSystemGeneric, SystemGuardianProtocol},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

use crate::{RemotingControl, RemotingControlHandle, RemotingExtensionConfig, RemotingExtensionId};

struct TestGuardian;

impl Actor for TestGuardian {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_actor_system() -> ActorSystemGeneric<NoStdToolbox> {
  let props = PropsGeneric::from_fn(|| TestGuardian).with_name("user-guardian");
  ActorSystemGeneric::new(&props).expect("actor system initialized")
}

struct CollectingSubscriber {
  events: ToolboxMutex<Vec<EventStreamEvent<NoStdToolbox>>, NoStdToolbox>,
}

impl CollectingSubscriber {
  fn new() -> ArcShared<Self> {
    ArcShared::new(Self {
      events: <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()),
    })
  }

  fn lifecycle_events(&self) -> Vec<RemotingLifecycleEvent> {
    self
      .events
      .lock()
      .iter()
      .filter_map(|event| match event {
        | EventStreamEvent::RemotingLifecycle(event) => Some(event.clone()),
        | _ => None,
      })
      .collect()
  }
}

impl EventStreamSubscriber for CollectingSubscriber {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

fn install_extension(
  system: &ActorSystemGeneric<NoStdToolbox>,
  config: RemotingExtensionConfig,
) -> (
  ArcShared<CollectingSubscriber>,
  RemotingControlHandle<NoStdToolbox>,
  EventStreamSubscriptionGeneric<NoStdToolbox>,
) {
  let stream = system.event_stream();
  let subscriber = CollectingSubscriber::new();
  let subscriber_ref: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber.clone();
  let subscription = EventStreamGeneric::subscribe_arc(&stream, &subscriber_ref);
  let id = RemotingExtensionId::new(config);
  let extension = system.register_extension(&id);
  let handle = extension.handle();
  (subscriber, handle, subscription)
}

#[test]
fn auto_start_publishes_start_event() {
  let system = build_actor_system();
  let (subscriber, _handle, _subscription) =
    install_extension(&system, RemotingExtensionConfig::default().with_auto_start(true));

  assert!(subscriber
    .lifecycle_events()
    .iter()
    .any(|event| matches!(event, RemotingLifecycleEvent::Starting)));
}

#[test]
fn manual_start_requires_explicit_invocation() {
  let system = build_actor_system();
  let (subscriber, handle, _subscription) =
    install_extension(&system, RemotingExtensionConfig::default().with_auto_start(false));

  assert!(subscriber.lifecycle_events().is_empty());

  let _ = handle.start();

  assert!(subscriber
    .lifecycle_events()
    .iter()
    .any(|event| matches!(event, RemotingLifecycleEvent::Starting)));
}

#[test]
fn termination_hook_publishes_shutdown_event() {
  let system = build_actor_system();
  let (subscriber, handle, _subscription) = install_extension(&system, RemotingExtensionConfig::default());

  let supervisor = handle.supervisor_ref().expect("supervisor registered");
  supervisor
    .tell(AnyMessageGeneric::new(SystemGuardianProtocol::<NoStdToolbox>::TerminationHook))
    .expect("hook delivery");

  assert!(subscriber
    .lifecycle_events()
    .iter()
    .any(|event| matches!(event, RemotingLifecycleEvent::Shutdown)));
}
