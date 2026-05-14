use std::{
  any::{Any, TypeId},
  sync::{
    Arc, Mutex,
    mpsc::{self, Sender},
  },
  time::{Duration, Instant},
};

use bytes::Bytes;
use fraktor_actor_adaptor_std_rs::{system::std_actor_system_config, tick_driver::TestTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, Pid,
    actor_path::{ActorPath, ActorPathParser},
    actor_ref::{ActorRef, NullSender},
    actor_ref_provider::LocalActorRefProviderInstaller,
    error::ActorError,
    extension::{ExtensionInstaller, ExtensionInstallers},
    messaging::{AnyMessage, AnyMessageView, system_message::SystemMessage},
    props::Props,
  },
  event::stream::{CorrelationId, EventStreamEvent, EventStreamSubscriber, RemotingLifecycleEvent, subscriber_handle},
  serialization::{
    SerializationCallScope, SerializationError, SerializationExtensionInstaller, SerializationSetupBuilder, Serializer,
    SerializerId, default_serialization_extension_id,
  },
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_remote_core_rs::{
  address::{Address, RemoteNodeId, UniqueAddress},
  association::QuarantineReason,
  config::RemoteConfig,
  envelope::{InboundEnvelope, OutboundPriority},
  extension::{Remote, RemoteEvent, RemoteShared, RemotingError},
  transport::TransportEndpoint,
  watcher::WatcherCommand,
  wire::{ControlPdu, WireFrame},
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess};
use tokio::{
  net::TcpStream,
  sync::mpsc::{self as tokio_mpsc, UnboundedSender},
  time::{sleep, timeout},
};

use crate::{
  extension_installer::remoting_extension_installer::{
    RemotingExtensionInstaller, RemotingRunState, deliver_inbound_envelope, forward_watcher_command_for_event,
    rollback_started_remote, spawn_watcher_task_with_state,
  },
  provider::StdRemoteActorRefProviderInstaller,
  tests::test_support_test::EventHarness,
  transport::tcp::TcpRemoteTransport,
};

struct RecordingBytesActor {
  tx: Sender<Bytes>,
}

struct RecordingEventSubscriber {
  tx: Sender<EventStreamEvent>,
}

struct CustomPayload;

struct StartRemoteWatch {
  target: ActorRef,
}

struct CustomPayloadSerializer {
  id: SerializerId,
}

struct RecordingTerminationActor {
  terminated_tx: UnboundedSender<Pid>,
  watched_tx:    UnboundedSender<()>,
}

impl RecordingBytesActor {
  fn new(tx: Sender<Bytes>) -> Self {
    Self { tx }
  }
}

impl RecordingEventSubscriber {
  fn new(tx: Sender<EventStreamEvent>) -> Self {
    Self { tx }
  }
}

impl CustomPayloadSerializer {
  const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl StartRemoteWatch {
  fn new(target: ActorRef) -> Self {
    Self { target }
  }
}

impl RecordingTerminationActor {
  fn new(terminated_tx: UnboundedSender<Pid>, watched_tx: UnboundedSender<()>) -> Self {
    Self { terminated_tx, watched_tx }
  }
}

impl Actor for RecordingBytesActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(bytes) = message.downcast_ref::<Bytes>() {
      self.tx.send(bytes.clone()).expect("recording channel should be open");
    }
    Ok(())
  }
}

impl Actor for RecordingTerminationActor {
  fn receive(&mut self, context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<StartRemoteWatch>() {
      context.watch(&command.target).expect("remote watch should install");
      self.watched_tx.send(()).expect("watch recording channel should be open");
    }
    Ok(())
  }

  fn on_terminated(&mut self, _context: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    self.terminated_tx.send(terminated).expect("termination recording channel should be open");
    Ok(())
  }
}

impl EventStreamSubscriber for RecordingEventSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.tx.send(event.clone()).expect("recording event channel should be open");
  }
}

impl Serializer for CustomPayloadSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    message.downcast_ref::<CustomPayload>().ok_or(SerializationError::InvalidFormat)?;
    Ok(vec![0xCA, 0xFE])
  }

  fn from_binary(
    &self,
    _bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Ok(Box::new(CustomPayload))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

#[test]
fn custom_payload_serializer_round_trips_custom_payload() {
  let serializer_id = SerializerId::try_from(700).expect("valid custom serializer id");
  let serializer = CustomPayloadSerializer::new(serializer_id);

  let bytes = serializer.to_binary(&CustomPayload).expect("custom payload should serialize");
  let payload = serializer.from_binary(&bytes, None).expect("custom payload should deserialize");

  assert_eq!(serializer.identifier(), serializer_id);
  assert_eq!(bytes, vec![0xCA, 0xFE]);
  assert!(payload.downcast::<CustomPayload>().is_ok());
  assert!(serializer.as_any().downcast_ref::<CustomPayloadSerializer>().is_some());
}

fn make_transport() -> TcpRemoteTransport {
  TcpRemoteTransport::new("127.0.0.1:0", Vec::new())
}

fn make_transport_with_addresses(addresses: Vec<Address>) -> TcpRemoteTransport {
  TcpRemoteTransport::new("127.0.0.1:0", addresses)
}

fn remote_config() -> RemoteConfig {
  RemoteConfig::new("127.0.0.1")
}

fn custom_serialization_installer() -> (SerializerId, SerializationExtensionInstaller) {
  let serializer_id = SerializerId::try_from(700).expect("valid custom serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(CustomPayloadSerializer::new(serializer_id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("custom", serializer_id, serializer)
    .expect("register custom serializer")
    .set_fallback("custom")
    .expect("set custom fallback")
    .bind::<CustomPayload>("custom")
    .expect("bind custom payload")
    .build()
    .expect("build custom serialization setup");
  (serializer_id, SerializationExtensionInstaller::new(setup))
}

fn make_remote(transport: TcpRemoteTransport) -> (Remote, EventHarness) {
  let harness = EventHarness::new();
  let serialization_extension = harness.system().extended().register_extension(&default_serialization_extension_id());
  let remote = Remote::new(transport, remote_config(), harness.publisher().clone(), serialization_extension);
  (remote, harness)
}

fn register_rejecting_temp_actor(system: &ActorSystem, pid: Pid) -> ActorPath {
  let actor_ref = ActorRef::with_canonical_path(pid, NullSender, ActorPath::root());
  let _name = system.state().register_temp_actor(actor_ref);
  system.state().canonical_actor_path(&pid).expect("temp actor path should be registered")
}

fn assert_configuration_error(error: ActorSystemBuildError, expected_message: &str) {
  match error {
    | ActorSystemBuildError::Configuration(message) => assert_eq!(message, expected_message),
    | other => panic!("expected configuration error, got {other:?}"),
  }
}

fn listen_started_authorities(events: &[EventStreamEvent]) -> Vec<String> {
  events
    .iter()
    .filter_map(|event| match event {
      | EventStreamEvent::RemotingLifecycle(RemotingLifecycleEvent::ListenStarted { authority, .. }) => {
        Some(authority.clone())
      },
      | _ => None,
    })
    .collect()
}

fn replayed_events(system: &ActorSystem) -> Vec<EventStreamEvent> {
  let (tx, rx) = mpsc::channel();
  let subscriber = subscriber_handle(RecordingEventSubscriber::new(tx));
  let _subscription = system.subscribe_event_stream(&subscriber);
  rx.try_iter().collect()
}

fn first_listen_started_port(system: &ActorSystem) -> u16 {
  let events = replayed_events(system);
  let authorities = listen_started_authorities(&events);
  authorities
    .first()
    .and_then(|authority| authority.rsplit(':').next())
    .expect("listen started authority port")
    .parse()
    .expect("listen started authority numeric port")
}

async fn terminate_system(system: &ActorSystem) {
  system.terminate().expect("terminate");
  timeout(Duration::from_secs(1), system.when_terminated()).await.expect("system should terminate");
}

async fn assert_listener_accepts(port: u16) {
  timeout(Duration::from_secs(1), TcpStream::connect(("127.0.0.1", port)))
    .await
    .expect("remote listener should accept connections")
    .expect("remote listener should be reachable");
}

async fn assert_listener_stops(port: u16) {
  timeout(Duration::from_secs(5), async {
    loop {
      if TcpStream::connect(("127.0.0.1", port)).await.is_err() {
        break;
      }
      sleep(Duration::from_millis(10)).await;
    }
  })
  .await
  .expect("remote listener should stop");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_lifecycle_starts_and_shuts_down() {
  let (mut remote, _harness) = make_remote(make_transport());
  assert!(!remote.lifecycle().is_running());

  remote.start().expect("start should succeed from Pending");
  assert!(remote.lifecycle().is_running());

  remote.shutdown().expect("shutdown should succeed from Running");
  assert!(remote.lifecycle().is_terminated());
}

#[test]
fn remote_shutdown_from_pending_terminates_without_error() {
  let (mut remote, _harness) = make_remote(make_transport());

  remote.shutdown().expect("shutdown should succeed from Pending");

  assert!(remote.lifecycle().is_terminated());
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_double_start_returns_already_running() {
  let (mut remote, _harness) = make_remote(make_transport());
  remote.start().expect("first start");
  let err = remote.start().unwrap_err();
  assert_eq!(err, RemotingError::AlreadyRunning);
  remote.shutdown().expect("shutdown after double-start check");
}

#[test]
fn remote_quarantine_requires_running_state() {
  let (mut remote, _harness) = make_remote(make_transport());
  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  let err = remote.quarantine(&address, Some(1), QuarantineReason::new("not started"), 1).unwrap_err();
  assert_eq!(err, RemotingError::NotStarted);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_quarantine_succeeds_while_running() {
  let (mut remote, _harness) = make_remote(make_transport());
  remote.start().expect("start");
  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  remote.quarantine(&address, Some(1), QuarantineReason::new("test"), 1).expect("quarantine while running");
  remote.shutdown().expect("shutdown after quarantine");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_start_snapshots_advertised_addresses() {
  let addresses = vec![Address::new("local-sys", "127.0.0.1", 2551), Address::new("local-sys", "127.0.0.2", 2552)];
  let (mut remote, _harness) = make_remote(make_transport_with_addresses(addresses.clone()));

  remote.start().expect("start should snapshot advertised addresses");

  assert_eq!(remote.addresses(), addresses.as_slice());
  remote.shutdown().expect("shutdown after snapshot check");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_start_publishes_listen_started_for_each_advertised_address() {
  let addresses = vec![Address::new("local-sys", "127.0.0.1", 2551), Address::new("local-sys", "127.0.0.2", 2552)];
  let (mut remote, harness) = make_remote(make_transport_with_addresses(addresses));

  remote.start().expect("start should publish listen events");

  harness.events_with(|events| {
    let mut authorities = listen_started_authorities(events);
    authorities.sort();
    assert_eq!(authorities, vec![String::from("local-sys@127.0.0.1:2551"), String::from("local-sys@127.0.0.2:2552")]);
    assert!(events.iter().any(|event| matches!(
      event,
      EventStreamEvent::RemotingLifecycle(RemotingLifecycleEvent::ListenStarted {
        correlation_id,
        ..
      }) if *correlation_id == CorrelationId::nil()
    )));
  });
  remote.shutdown().expect("shutdown after listen event check");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_config_install_starts_listener() {
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let installer = ArcShared::new(RemotingExtensionInstaller::new(
    TcpRemoteTransport::new("127.0.0.1:0", vec![listen_address]),
    remote_config(),
  ));
  let installers = ExtensionInstallers::default().with_shared_extension_installer(installer.clone());
  let config = std_actor_system_config(TestTickDriver::default()).with_extension_installers(installers);
  let system = ActorSystem::create_with_noop_guardian(config).expect("system should install remoting extension");
  let port = first_listen_started_port(&system);

  assert_listener_accepts(port).await;
  terminate_system(&system).await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_system_termination_shuts_down_remote() {
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let installer = ArcShared::new(RemotingExtensionInstaller::new(
    TcpRemoteTransport::new("127.0.0.1:0", vec![listen_address]),
    remote_config(),
  ));
  let installers = ExtensionInstallers::default().with_shared_extension_installer(installer.clone());
  let config = std_actor_system_config(TestTickDriver::default()).with_extension_installers(installers);
  let system = ActorSystem::create_with_noop_guardian(config).expect("system should install remoting extension");
  let port = first_listen_started_port(&system);

  assert_listener_accepts(port).await;
  system.terminate().expect("terminate should trigger remoting shutdown");
  timeout(Duration::from_secs(5), system.when_terminated()).await.expect("system should terminate");
  assert_listener_stops(port).await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_double_install_returns_configuration_error() {
  let installer = RemotingExtensionInstaller::new(make_transport(), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("first install should create remote");

  let error = installer.install(harness.system()).expect_err("second install should fail");

  assert_configuration_error(error, "remote extension is already installed");
  terminate_system(harness.system()).await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_reuses_preinstalled_custom_serialization_extension() {
  let (serializer_id, serialization_installer) = custom_serialization_installer();
  let remoting_installer = ArcShared::new(RemotingExtensionInstaller::new(
    TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)]),
    remote_config(),
  ));
  let installers = ExtensionInstallers::default()
    .with_extension_installer(serialization_installer)
    .with_shared_extension_installer(remoting_installer);
  let config = std_actor_system_config(TestTickDriver::default()).with_extension_installers(installers);
  let system = ActorSystem::create_with_noop_guardian(config).expect("system should install extensions");
  let serialization_extension = system.extended().register_extension(&default_serialization_extension_id());

  let serialized = serialization_extension
    .with_read(|extension| extension.serialize(&CustomPayload, SerializationCallScope::Remote))
    .expect("preinstalled custom serializer should be reused by remoting");

  assert_eq!(serialized.serializer_id(), serializer_id);
  terminate_system(&system).await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn forward_watcher_command_logs_when_queue_is_full() {
  let (watcher_tx, mut watcher_rx) = tokio_mpsc::channel(1);
  watcher_tx
    .try_send(WatcherCommand::HeartbeatReceived { from: Address::new("queued", "127.0.0.1", 2551), now: 1 })
    .expect("watcher queue should accept first command");
  let event = RemoteEvent::InboundFrameReceived {
    authority: TransportEndpoint::new("remote-sys@10.0.0.1:2552"),
    frame:     WireFrame::Control(ControlPdu::Heartbeat { authority: String::from("remote-sys@10.0.0.1:2552") }),
    now_ms:    2,
  };

  forward_watcher_command_for_event(&event, &watcher_tx);

  assert!(matches!(watcher_rx.try_recv(), Ok(WatcherCommand::HeartbeatReceived { now: 1, .. })));
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn spawn_watcher_task_rejects_duplicate_handle() {
  let mut run_state = RemotingRunState::new();
  let (_command_tx, command_rx) = tokio_mpsc::channel(1);
  let (event_tx, _event_rx) = tokio_mpsc::channel(1);
  let system = ActorSystem::create_with_noop_guardian(std_actor_system_config(TestTickDriver::default()))
    .expect("actor system should build");
  spawn_watcher_task_with_state(
    &mut run_state,
    command_rx,
    event_tx.clone(),
    system.clone(),
    Address::new("local-sys", "127.0.0.1", 2551),
    Instant::now(),
    Duration::from_millis(50),
  )
  .expect("first watcher task should spawn");
  let (_second_command_tx, second_command_rx) = tokio_mpsc::channel(1);

  let error = spawn_watcher_task_with_state(
    &mut run_state,
    second_command_rx,
    event_tx,
    system,
    Address::new("local-sys", "127.0.0.1", 2551),
    Instant::now(),
    Duration::from_millis(50),
  )
  .expect_err("second watcher task should be rejected");

  assert_eq!(error, RemotingError::AlreadyRunning);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn rollback_started_remote_aborts_watcher_task() {
  let mut run_state = RemotingRunState::new();
  let (_command_tx, command_rx) = tokio_mpsc::channel(1);
  let (event_tx, _event_rx) = tokio_mpsc::channel(1);
  let (remote, harness) = make_remote(make_transport());
  spawn_watcher_task_with_state(
    &mut run_state,
    command_rx,
    event_tx.clone(),
    harness.system().clone(),
    Address::new("local-sys", "127.0.0.1", 2551),
    Instant::now(),
    Duration::from_millis(50),
  )
  .expect("watcher task should spawn before rollback");
  let run_state = Arc::new(Mutex::new(run_state));
  let remote = RemoteShared::new(remote);

  rollback_started_remote(&remote, &event_tx, &run_state);

  let (_second_command_tx, second_command_rx) = tokio_mpsc::channel(1);
  spawn_watcher_task_with_state(
    &mut run_state.lock().expect("run state should lock after rollback"),
    second_command_rx,
    event_tx,
    harness.system().clone(),
    Address::new("local-sys", "127.0.0.1", 2551),
    Instant::now(),
    Duration::from_millis(50),
  )
  .expect("rollback should clear the watcher handle");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn custom_serialization_installer_must_precede_remoting_installer() {
  let (_serializer_id, serialization_installer) = custom_serialization_installer();
  let remoting_installer = ArcShared::new(RemotingExtensionInstaller::new(
    TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)]),
    remote_config(),
  ));
  let installers = ExtensionInstallers::default()
    .with_shared_extension_installer(remoting_installer)
    .with_extension_installer(serialization_installer);
  let config = std_actor_system_config(TestTickDriver::default()).with_extension_installers(installers);
  let system = ActorSystem::create_with_noop_guardian(config).expect("system should install extensions");
  let serialization_extension = system.extended().register_extension(&default_serialization_extension_id());

  let error = serialization_extension
    .with_read(|extension| extension.serialize(&CustomPayload, SerializationCallScope::Remote))
    .expect_err("late custom serialization installer should not replace the remoting registry");

  assert!(matches!(error, SerializationError::InvalidFormat | SerializationError::NotSerializable(_)));
  terminate_system(&system).await;
}

#[test]
fn inbound_delivery_bridge_sends_bytes_payload_to_local_actor() {
  let config = std_actor_system_config(TestTickDriver::default())
    .with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default());
  let system = ActorSystem::create_with_noop_guardian(config).expect("noop actor system should build");
  let (tx, rx) = mpsc::channel();
  let props = Props::from_fn({
    let tx = tx.clone();
    move || RecordingBytesActor::new(tx.clone())
  });
  let child = system.actor_of_named(&props, "remote-target").expect("spawn recording actor");
  let recipient = child.actor_ref().path().expect("recording actor path");
  let envelope = InboundEnvelope::new(
    recipient,
    RemoteNodeId::new("remote-sys", "127.0.0.1", Some(2552), 1),
    AnyMessage::new(Bytes::from_static(b"inbound payload")),
    None,
    CorrelationId::nil(),
    OutboundPriority::User,
  );

  deliver_inbound_envelope(envelope, &system);

  let received = rx.recv_timeout(Duration::from_secs(1)).expect("local actor should receive inbound remote payload");
  assert_eq!(received, Bytes::from_static(b"inbound payload"));
}

#[test]
fn inbound_delivery_bridge_applies_remote_watch_and_unwatch_messages() {
  let system = ActorSystem::create_with_noop_guardian(std_actor_system_config(TestTickDriver::default()))
    .expect("actor system should build");
  let props = Props::from_fn(|| RecordingBytesActor::new(mpsc::channel().0));
  let target = system.actor_of_named(&props, "watch-target").expect("target actor");
  let watcher = system.actor_of_named(&props, "watcher").expect("watcher actor");
  let target_path = target.actor_ref().path().expect("target path");
  let watcher_path = watcher.actor_ref().path().expect("watcher path");

  deliver_inbound_envelope(
    InboundEnvelope::new(
      target_path.clone(),
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::Watch(Pid::new(0, 0))),
      Some(watcher_path.clone()),
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );
  assert!(system.state().cell(&target.actor_ref().pid()).is_some());
  assert!(system.state().cell(&watcher.actor_ref().pid()).is_some());

  deliver_inbound_envelope(
    InboundEnvelope::new(
      target_path,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::Unwatch(Pid::new(0, 0))),
      Some(watcher_path),
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );
  assert!(system.state().cell(&target.actor_ref().pid()).is_some());
  assert!(system.state().cell(&watcher.actor_ref().pid()).is_some());
}

#[test]
fn inbound_delivery_bridge_drops_remote_watch_when_sender_is_missing() {
  let system = ActorSystem::create_with_noop_guardian(std_actor_system_config(TestTickDriver::default()))
    .expect("actor system should build");
  let props = Props::from_fn(|| RecordingBytesActor::new(mpsc::channel().0));
  let target = system.actor_of_named(&props, "watch-missing-sender").expect("target actor");
  let target_path = target.actor_ref().path().expect("target path");

  deliver_inbound_envelope(
    InboundEnvelope::new(
      target_path,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::Watch(Pid::new(0, 0))),
      None,
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );
}

#[test]
fn inbound_delivery_bridge_drops_remote_watch_when_recipient_or_sender_is_unresolved() {
  let system = ActorSystem::create_with_noop_guardian(std_actor_system_config(TestTickDriver::default()))
    .expect("actor system should build");
  let props = Props::from_fn(|| RecordingBytesActor::new(mpsc::channel().0));
  let target = system.actor_of_named(&props, "watch-present-target").expect("target actor");
  let target_path = target.actor_ref().path().expect("target path");
  let missing_target = ActorPath::root().child("user").child("missing-target");
  let missing_watcher = ActorPath::root().child("user").child("missing-watcher");

  deliver_inbound_envelope(
    InboundEnvelope::new(
      missing_target,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::Watch(Pid::new(0, 0))),
      Some(target_path.clone()),
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );
  deliver_inbound_envelope(
    InboundEnvelope::new(
      target_path,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::Watch(Pid::new(0, 0))),
      Some(missing_watcher),
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );
}

#[test]
fn inbound_delivery_bridge_records_failed_remote_watch_delivery() {
  let system = ActorSystem::create_with_noop_guardian(std_actor_system_config(TestTickDriver::default()))
    .expect("actor system should build");
  let target_path = register_rejecting_temp_actor(&system, Pid::new(7100, 0));
  let props = Props::from_fn(|| RecordingBytesActor::new(mpsc::channel().0));
  let watcher = system.actor_of_named(&props, "watcher-send-error").expect("watcher actor");
  let watcher_path = watcher.actor_ref().path().expect("watcher path");

  deliver_inbound_envelope(
    InboundEnvelope::new(
      target_path,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::Watch(Pid::new(0, 0))),
      Some(watcher_path),
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );

  assert!(!system.dead_letters().is_empty());
}

#[test]
fn inbound_delivery_bridge_drops_remote_deathwatch_when_sender_or_watcher_is_unresolved() {
  let system = ActorSystem::create_with_noop_guardian(std_actor_system_config(TestTickDriver::default()))
    .expect("actor system should build");
  let props = Props::from_fn(|| RecordingBytesActor::new(mpsc::channel().0));
  let watcher = system.actor_of_named(&props, "deathwatch-present-watcher").expect("watcher actor");
  let watcher_path = watcher.actor_ref().path().expect("watcher path");
  let missing_watcher = ActorPath::root().child("user").child("deathwatch-missing-watcher");
  let missing_target = ActorPath::root().child("user").child("deathwatch-missing-target");

  deliver_inbound_envelope(
    InboundEnvelope::new(
      watcher_path.clone(),
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(0, 0))),
      None,
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );
  deliver_inbound_envelope(
    InboundEnvelope::new(
      missing_watcher,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(0, 0))),
      Some(watcher_path.clone()),
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );
  deliver_inbound_envelope(
    InboundEnvelope::new(
      watcher_path,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(0, 0))),
      Some(missing_target),
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );
}

#[test]
fn inbound_delivery_bridge_records_failed_remote_deathwatch_delivery() {
  let system = ActorSystem::create_with_noop_guardian(std_actor_system_config(TestTickDriver::default()))
    .expect("actor system should build");
  let watcher_path = register_rejecting_temp_actor(&system, Pid::new(7200, 0));
  let props = Props::from_fn(|| RecordingBytesActor::new(mpsc::channel().0));
  let target = system.actor_of_named(&props, "deathwatch-send-error-target").expect("target actor");
  let target_path = target.actor_ref().path().expect("target path");

  deliver_inbound_envelope(
    InboundEnvelope::new(
      watcher_path,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(0, 0))),
      Some(target_path),
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );

  assert!(!system.dead_letters().is_empty());
}

#[test]
fn inbound_delivery_bridge_routes_other_system_messages_to_recipient_or_dead_letters() {
  let system = ActorSystem::create_with_noop_guardian(std_actor_system_config(TestTickDriver::default()))
    .expect("actor system should build");
  let props = Props::from_fn(|| RecordingBytesActor::new(mpsc::channel().0));
  let recipient = system.actor_of_named(&props, "system-recipient").expect("recipient actor");
  let recipient_path = recipient.actor_ref().path().expect("recipient path");
  let missing_recipient = ActorPath::root().child("user").child("system-missing");

  deliver_inbound_envelope(
    InboundEnvelope::new(
      recipient_path,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::Suspend),
      None,
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );
  deliver_inbound_envelope(
    InboundEnvelope::new(
      missing_recipient,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::Resume),
      None,
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );
}

#[test]
fn inbound_delivery_bridge_records_failed_generic_system_delivery() {
  let system = ActorSystem::create_with_noop_guardian(std_actor_system_config(TestTickDriver::default()))
    .expect("actor system should build");
  let recipient_path = register_rejecting_temp_actor(&system, Pid::new(7300, 0));

  deliver_inbound_envelope(
    InboundEnvelope::new(
      recipient_path,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::Suspend),
      None,
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );

  assert!(!system.dead_letters().is_empty());
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn inbound_delivery_bridge_deduplicates_remote_deathwatch_notification() {
  let local_address = Address::new("local-sys", "127.0.0.1", 2551);
  let remoting_installer = ArcShared::new(RemotingExtensionInstaller::new(
    TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)]),
    remote_config(),
  ));
  let installers = ExtensionInstallers::default().with_shared_extension_installer(remoting_installer.clone());
  let provider_installer = StdRemoteActorRefProviderInstaller::from_remoting_extension_installer(
    UniqueAddress::new(local_address, 7),
    remoting_installer,
  );
  let config = std_actor_system_config(TestTickDriver::default())
    .with_system_name("local-sys")
    .with_extension_installers(installers)
    .with_actor_ref_provider_installer(provider_installer);
  let system = ActorSystem::create_with_noop_guardian(config).expect("actor system should build");
  let (terminated_tx, mut terminated_rx) = tokio_mpsc::unbounded_channel();
  let (watched_tx, mut watched_rx) = tokio_mpsc::unbounded_channel();
  let props = Props::from_fn({
    let terminated_tx = terminated_tx.clone();
    let watched_tx = watched_tx.clone();
    move || RecordingTerminationActor::new(terminated_tx.clone(), watched_tx.clone())
  });
  let watcher = system.actor_of_named(&props, "remote-watcher").expect("watcher actor should spawn");
  let watcher_path = watcher.actor_ref().path().expect("watcher path");
  assert_eq!(system.pid_by_path(&watcher_path), Some(watcher.actor_ref().pid()));
  let remote_target_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/target")
    .expect("remote target path should parse");
  let remote_target = system.resolve_actor_ref(remote_target_path.clone()).expect("remote target should resolve");
  assert_eq!(
    system.resolve_actor_ref(remote_target_path.clone()).expect("remote target should stay cached").pid(),
    remote_target.pid()
  );
  let mut watcher_ref = watcher.actor_ref().clone();

  watcher_ref
    .try_tell(AnyMessage::new(StartRemoteWatch::new(remote_target.clone())))
    .expect("watch command should reach local actor");
  timeout(Duration::from_secs(1), watched_rx.recv()).await.expect("watch should be installed").expect("watch channel");
  assert!(
    system
      .state()
      .cell(&watcher.actor_ref().pid())
      .expect("watcher cell should exist")
      .is_watching(remote_target.pid()),
    "watcher should track remote target before inbound notification"
  );

  deliver_inbound_envelope(
    InboundEnvelope::new(
      watcher_path.clone(),
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(0, 0))),
      Some(remote_target_path.clone()),
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );
  let terminated = timeout(Duration::from_secs(1), terminated_rx.recv())
    .await
    .expect("termination should be delivered once")
    .expect("termination channel");
  assert_eq!(terminated, remote_target.pid());
  deliver_inbound_envelope(
    InboundEnvelope::new(
      watcher_path,
      RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
      AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(0, 0))),
      Some(remote_target_path),
      CorrelationId::nil(),
      OutboundPriority::System,
    ),
    &system,
  );
  assert!(timeout(Duration::from_millis(50), terminated_rx.recv()).await.is_err());
  terminate_system(&system).await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_install_wires_listen_event_publisher() {
  let listen_address = Address::new("local-sys", "127.0.0.1", 2551);
  let installer = RemotingExtensionInstaller::new(make_transport_with_addresses(vec![listen_address]), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should wire event publisher");

  let events = harness.events();
  assert_eq!(listen_started_authorities(&events), vec![String::from("local-sys@127.0.0.1:2551")]);
  terminate_system(harness.system()).await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_start_binds_listener_and_publishes_actual_bound_port() {
  // Given
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let installer = RemotingExtensionInstaller::new(make_transport_with_addresses(vec![listen_address]), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should wire event publisher");

  // When
  let events = harness.events();
  let authorities = listen_started_authorities(&events);
  let authority = authorities.first().expect("listen started authority");
  let actual_port = authority.rsplit(':').next().expect("port").parse::<u16>().expect("numeric port");

  // Then
  assert_ne!(actual_port, 0);
  assert_eq!(listen_started_authorities(&events), vec![alloc::format!("local-sys@127.0.0.1:{actual_port}")]);

  terminate_system(harness.system()).await;
}
