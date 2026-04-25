#![cfg(not(target_os = "none"))]

use std::{boxed::Box, string::ToString, vec::Vec};

use fraktor_actor_adaptor_std_rs::std::tick_driver::StdTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, error::ActorError, extension::ExtensionInstaller, messaging::AnyMessageView, props::Props,
    setup::ActorSystemConfig,
  },
  event::stream::{EventStreamEvent, EventStreamSubscriber, EventStreamSubscriberShared, RemotingLifecycleEvent},
  system::ActorSystem,
};
use fraktor_remote_adaptor_std_rs::{
  extension_installer::RemotingExtensionInstaller, tcp_transport::TcpRemoteTransport,
};
use fraktor_remote_core_rs::{address::Address, extension::Remoting};
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock, SpinSyncMutex};

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct RecordingSubscriber {
  events: SharedLock<Vec<EventStreamEvent>>,
}

impl RecordingSubscriber {
  fn new(events: SharedLock<Vec<EventStreamEvent>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.with_lock(|events| events.push(event.clone()));
  }
}

fn subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  EventStreamSubscriberShared::from_shared_lock(SharedLock::new_with_driver::<SpinSyncMutex<_>>(Box::new(subscriber)))
}

fn main() {
  let props = Props::from_fn(|| NoopActor);
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");

  let events = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);

  let advertised_address = Address::new("remote-showcase", "127.0.0.1", 2551);
  let transport = SharedLock::new_with_driver::<DefaultMutex<_>>(TcpRemoteTransport::new("127.0.0.1:0", vec![
    advertised_address.clone(),
  ]));
  let installer = RemotingExtensionInstaller::new(transport);

  installer.install(&system).expect("remote extension install");
  let remoting = installer.remoting().expect("installed remoting handle");
  remoting.with_lock(|remoting| {
    remoting.start().expect("remote lifecycle start");
    assert_eq!(remoting.addresses(), core::slice::from_ref(&advertised_address));
    remoting.shutdown().expect("remote lifecycle shutdown");
  });

  let expected_authority = advertised_address.to_string();
  assert!(events.with_lock(|events| {
    events.iter().any(|event| {
      matches!(
        event,
        EventStreamEvent::RemotingLifecycle(RemotingLifecycleEvent::ListenStarted {
          authority,
          ..
        }) if authority == &expected_authority
      )
    })
  }));

  system.terminate().expect("terminate");
}
