use std::time::{Duration, Instant};

use fraktor_actor_adaptor_std_rs::{
  system::{create_noop_actor_system, std_actor_system_config},
  tick_driver::TestTickDriver,
};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_path::{ActorPath, ActorPathParser},
    actor_ref::ActorRef,
    actor_ref_provider::LocalActorRefProviderInstaller,
    messaging::{AnyMessage, system_message::SystemMessage},
  },
  event::stream::{ClassifierKey, CorrelationId, EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  serialization::default_serialization_extension_id,
  system::ActorSystem,
};
use fraktor_remote_core_rs::{
  address::{Address, RemoteNodeId, UniqueAddress},
  association::QuarantineReason,
  config::RemoteConfig,
  envelope::{OutboundEnvelope, OutboundPriority},
  extension::{EventPublisher, Remote, RemoteEvent, RemoteShared},
  transport::{RemoteTransport, TransportEndpoint, TransportError},
  watcher::WatcherEffect,
  wire::{AckPdu, ControlPdu, HandshakePdu, HandshakeRsp},
};
use tokio::{
  sync::mpsc::{self, UnboundedSender},
  time::timeout,
};

use super::{
  apply_effects, notify_local_watchers, run_watcher_task, send_heartbeat, send_redelivery_tick, send_system_envelope,
  try_apply_effects,
};

struct NoopRemoteTransport {
  addresses: Vec<Address>,
}

impl NoopRemoteTransport {
  fn new(addresses: Vec<Address>) -> Self {
    Self { addresses }
  }
}

impl RemoteTransport for NoopRemoteTransport {
  fn start(&mut self) -> Result<(), TransportError> {
    Ok(())
  }

  fn shutdown(&mut self) -> Result<(), TransportError> {
    Ok(())
  }

  fn connect_peer(&mut self, _remote: &Address) -> Result<(), TransportError> {
    Ok(())
  }

  fn send(&mut self, _envelope: OutboundEnvelope) -> Result<(), (TransportError, Box<OutboundEnvelope>)> {
    Ok(())
  }

  fn send_control(&mut self, _remote: &Address, _pdu: ControlPdu) -> Result<(), TransportError> {
    Ok(())
  }

  fn send_flush_request(&mut self, _remote: &Address, _pdu: ControlPdu, _lane_id: u32) -> Result<(), TransportError> {
    Ok(())
  }

  fn send_ack(&mut self, _remote: &Address, _pdu: AckPdu) -> Result<(), TransportError> {
    Ok(())
  }

  fn send_handshake(&mut self, _remote: &Address, _pdu: HandshakePdu) -> Result<(), TransportError> {
    Ok(())
  }

  fn schedule_handshake_timeout(
    &mut self,
    _authority: &TransportEndpoint,
    _timeout: Duration,
    _generation: u64,
  ) -> Result<(), TransportError> {
    Ok(())
  }

  fn addresses(&self) -> &[Address] {
    &self.addresses
  }

  fn default_address(&self) -> Option<&Address> {
    self.addresses.first()
  }

  fn local_address_for_remote(&self, _remote: &Address) -> Option<&Address> {
    self.default_address()
  }

  fn quarantine(
    &mut self,
    _address: &Address,
    _uid: Option<u64>,
    _reason: QuarantineReason,
  ) -> Result<(), TransportError> {
    Ok(())
  }
}

struct RecordingEventSubscriber {
  sender: UnboundedSender<EventStreamEvent>,
}

impl RecordingEventSubscriber {
  fn new(sender: UnboundedSender<EventStreamEvent>) -> Self {
    Self { sender }
  }
}

impl EventStreamSubscriber for RecordingEventSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if self.sender.send(event.clone()).is_err() {}
  }
}

fn local_address() -> Address {
  Address::new("local-sys", "127.0.0.1", 2551)
}

fn remote_address() -> Address {
  Address::new("remote-sys", "10.0.0.1", 2552)
}

fn remote_path(name: &str) -> ActorPath {
  ActorPathParser::parse(&alloc::format!("fraktor.tcp://remote-sys@10.0.0.1:2552/user/{name}")).expect("parse")
}

fn local_path(name: &str) -> ActorPath {
  ActorPath::root().child("user").child(name)
}

fn local_actor_system() -> ActorSystem {
  let config = std_actor_system_config(TestTickDriver::default())
    .with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default());
  ActorSystem::create_with_noop_guardian(config).expect("actor system should build")
}

fn remote_shared() -> RemoteShared {
  let system = create_noop_actor_system();
  RemoteShared::new(Remote::new(
    NoopRemoteTransport::new(vec![local_address()]),
    RemoteConfig::new("127.0.0.1"),
    EventPublisher::new(system.downgrade()),
    system.extended().register_extension(&default_serialization_extension_id()),
  ))
}

fn noop_transport_envelope(remote: &Address) -> OutboundEnvelope {
  OutboundEnvelope::new(
    remote_path("transport-target"),
    None,
    AnyMessage::new(String::from("payload")),
    OutboundPriority::User,
    RemoteNodeId::new(remote.system(), remote.host(), Some(remote.port()), 1),
    CorrelationId::nil(),
  )
}

fn user_guardian_path(system: &ActorSystem) -> ActorPath {
  system.user_guardian_ref().path().expect("user guardian path")
}

#[test]
fn noop_remote_transport_fixture_handles_watcher_contract_without_io() {
  let local = local_address();
  let remote = remote_address();
  let mut transport = NoopRemoteTransport::new(vec![local.clone()]);

  assert_eq!(transport.addresses(), [local.clone()].as_slice());
  assert_eq!(transport.default_address(), Some(&local));
  assert_eq!(transport.local_address_for_remote(&remote), Some(&local));

  transport.start().expect("start should be a no-op");
  transport.connect_peer(&remote).expect("connect should be a no-op");
  transport.send(noop_transport_envelope(&remote)).expect("send should be a no-op");
  let control = ControlPdu::Shutdown { authority: local.to_string() };
  transport.send_control(&remote, control.clone()).expect("control send should be a no-op");
  transport.send_flush_request(&remote, control, 0).expect("flush request should be a no-op");
  transport.send_ack(&remote, AckPdu::new(1, 1, 0)).expect("ack should be a no-op");
  transport
    .send_handshake(&remote, HandshakePdu::Rsp(HandshakeRsp::new(UniqueAddress::new(local.clone(), 1))))
    .expect("handshake should be a no-op");
  transport
    .schedule_handshake_timeout(&TransportEndpoint::new(remote.to_string()), Duration::from_millis(1), 1)
    .expect("handshake timeout should be a no-op");
  transport.quarantine(&remote, Some(1), QuarantineReason::new("test")).expect("quarantine should be a no-op");
  transport.shutdown().expect("shutdown should be a no-op");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn run_watcher_task_returns_when_command_channel_closes() {
  let (command_tx, command_rx) = mpsc::channel(1);
  let (event_tx, _event_rx) = mpsc::channel(8);
  drop(command_tx);

  timeout(
    Duration::from_secs(1),
    run_watcher_task(
      command_rx,
      remote_shared(),
      event_tx,
      create_noop_actor_system(),
      local_address(),
      Instant::now(),
      Duration::from_millis(10),
    ),
  )
  .await
  .expect("watcher task should exit when command channel closes");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn apply_effects_emits_remote_events_for_watch_heartbeat_and_rewatch() {
  let (event_tx, mut event_rx) = mpsc::channel(8);
  let system = local_actor_system();
  let (stream_tx, mut stream_rx) = mpsc::unbounded_channel();
  let subscriber = subscriber_handle(RecordingEventSubscriber::new(stream_tx));
  let _subscription = system.event_stream().subscribe_with_key(ClassifierKey::AddressTerminated, &subscriber);
  let target = remote_path("target");
  let watcher = local_path("watcher");
  let remote = remote_address();
  let local_target_path = user_guardian_path(&system);
  let local_watcher_path = local_target_path.clone();

  apply_effects(
    alloc::vec![
      WatcherEffect::SendWatch { target: target.clone(), watcher: watcher.clone() },
      WatcherEffect::SendUnwatch { target: target.clone(), watcher: watcher.clone() },
      WatcherEffect::SendHeartbeat { to: remote.clone() },
      WatcherEffect::NotifyTerminated { target: local_target_path, watchers: alloc::vec![local_watcher_path] },
      WatcherEffect::AddressTerminated {
        node:               remote.clone(),
        reason:             String::from("Deemed unreachable by remote failure detector"),
        observed_at_millis: 42,
      },
      WatcherEffect::NotifyQuarantined { node: remote.clone() },
      WatcherEffect::RewatchRemoteTargets { node: remote.clone(), watches: alloc::vec![(target, watcher)] },
    ],
    &event_tx,
    &system,
    &local_address(),
    Instant::now(),
    42,
  )
  .await;

  let mut events = alloc::vec![];
  while let Ok(event) = event_rx.try_recv() {
    events.push(event);
  }
  assert_eq!(events.len(), 5);
  assert!(events.iter().any(|event| matches!(
    event,
    RemoteEvent::OutboundControl {
      remote: event_remote,
      pdu: ControlPdu::Heartbeat { authority },
      now_ms: 42,
    } if event_remote == &remote && authority == "local-sys@127.0.0.1:2551"
  )));
  assert!(events.iter().any(|event| matches!(
    event,
    RemoteEvent::RedeliveryTimerFired {
      authority,
      now_ms: 42,
    } if authority.authority() == "remote-sys@10.0.0.1:2552"
  )));
  assert_eq!(events.iter().filter(|event| matches!(event, RemoteEvent::OutboundEnqueued { .. })).count(), 3);

  let address_event = stream_rx.try_recv().expect("address termination should be published");
  assert!(matches!(
    address_event,
    EventStreamEvent::AddressTerminated(event)
      if event.authority() == "remote-sys@10.0.0.1:2552"
        && event.reason() == "Deemed unreachable by remote failure detector"
        && event.observed_at_millis() == 42
  ));
  assert!(stream_rx.try_recv().is_err());
}

#[test]
fn try_apply_effects_emits_remote_events_without_awaiting() {
  let (event_tx, mut event_rx) = mpsc::channel(8);
  let system = local_actor_system();
  let (stream_tx, mut stream_rx) = mpsc::unbounded_channel();
  let subscriber = subscriber_handle(RecordingEventSubscriber::new(stream_tx));
  let _subscription = system.event_stream().subscribe_with_key(ClassifierKey::AddressTerminated, &subscriber);
  let target = remote_path("target");
  let watcher = local_path("watcher");
  let remote = remote_address();
  let local_target_path = user_guardian_path(&system);
  let local_watcher_path = local_target_path.clone();

  try_apply_effects(
    alloc::vec![
      WatcherEffect::SendWatch { target: target.clone(), watcher: watcher.clone() },
      WatcherEffect::SendUnwatch { target: target.clone(), watcher: watcher.clone() },
      WatcherEffect::SendHeartbeat { to: remote.clone() },
      WatcherEffect::NotifyTerminated { target: local_target_path, watchers: alloc::vec![local_watcher_path] },
      WatcherEffect::AddressTerminated {
        node:               remote.clone(),
        reason:             String::from("Deemed unreachable by remote failure detector"),
        observed_at_millis: 42,
      },
      WatcherEffect::NotifyQuarantined { node: remote.clone() },
      WatcherEffect::RewatchRemoteTargets { node: remote.clone(), watches: alloc::vec![(target, watcher)] },
    ],
    &event_tx,
    &system,
    &local_address(),
    Instant::now(),
    42,
  );

  let mut events = alloc::vec![];
  while let Ok(event) = event_rx.try_recv() {
    events.push(event);
  }
  assert_eq!(events.len(), 5);
  assert!(events.iter().any(|event| matches!(
    event,
    RemoteEvent::OutboundControl {
      remote: event_remote,
      pdu: ControlPdu::Heartbeat { authority },
      now_ms: 42,
    } if event_remote == &remote && authority == "local-sys@127.0.0.1:2551"
  )));
  assert_eq!(events.iter().filter(|event| matches!(event, RemoteEvent::OutboundEnqueued { .. })).count(), 3);
  assert!(stream_rx.try_recv().is_ok());
}

#[test]
fn try_apply_effects_logs_and_returns_when_event_queue_is_full_or_recipient_is_local() {
  let (event_tx, mut event_rx) = mpsc::channel(1);
  let system = local_actor_system();
  event_tx.try_send(RemoteEvent::TransportShutdown).expect("queue should be full after seed event");

  try_apply_effects(
    alloc::vec![
      WatcherEffect::SendHeartbeat { to: remote_address() },
      WatcherEffect::SendWatch { target: remote_path("full"), watcher: local_path("watcher") },
      WatcherEffect::SendUnwatch { target: local_path("not-remote"), watcher: local_path("watcher") },
    ],
    &event_tx,
    &system,
    &local_address(),
    Instant::now(),
    42,
  );

  assert!(matches!(event_rx.try_recv(), Ok(RemoteEvent::TransportShutdown)));
  assert!(event_rx.try_recv().is_err());
}

#[test]
fn notify_local_watchers_returns_when_target_cannot_be_resolved() {
  let system = local_actor_system();

  notify_local_watchers(&system, local_path("missing-target"), alloc::vec![local_path("watcher")]);
}

#[test]
fn notify_local_watchers_skips_unresolved_watcher() {
  let system = local_actor_system();
  let target_path = user_guardian_path(&system);
  assert!(system.resolve_actor_ref(target_path.clone()).is_ok());

  notify_local_watchers(&system, target_path, alloc::vec![local_path("missing-watcher")]);
}

#[test]
fn notify_local_watchers_logs_when_send_fails() {
  let system = local_actor_system();
  let target_path = user_guardian_path(&system);
  let failing_watcher = ActorRef::null();
  let failing_watcher_pid = failing_watcher.pid();
  let _name = system.state().register_temp_actor(failing_watcher);
  let failing_watcher_path = system.state().canonical_actor_path(&failing_watcher_pid).expect("failing watcher path");

  notify_local_watchers(&system, target_path, alloc::vec![failing_watcher_path]);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn watcher_send_helpers_log_and_return_when_event_receiver_is_closed() {
  let (event_tx, event_rx) = mpsc::channel(1);
  drop(event_rx);

  send_heartbeat(&event_tx, &local_address(), remote_address(), 10).await;
  send_redelivery_tick(&event_tx, remote_address(), 11).await;
  send_system_envelope(&event_tx, remote_path("closed"), None, SystemMessage::Watch(Pid::new(0, 0)), Instant::now())
    .await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn send_system_envelope_does_not_enqueue_event_for_local_recipient() {
  let (event_tx, mut event_rx) = mpsc::channel(1);

  send_system_envelope(
    &event_tx,
    local_path("not-remote"),
    None,
    SystemMessage::Unwatch(Pid::new(0, 0)),
    Instant::now(),
  )
  .await;

  assert!(event_rx.try_recv().is_err());
}
