use std::{
  sync::mpsc::{self, Sender},
  time::Duration,
};

use bytes::Bytes;
use fraktor_actor_adaptor_std_rs::std::{system::std_actor_system_config, tick_driver::TestTickDriver};
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    actor_ref_provider::LocalActorRefProviderInstaller,
    error::ActorError,
    extension::{ExtensionInstaller, ExtensionInstallers},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  event::stream::{CorrelationId, EventStreamEvent, RemotingLifecycleEvent},
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_remote_core_rs::core::{
  address::{Address, RemoteNodeId},
  association::QuarantineReason,
  config::RemoteConfig,
  envelope::{InboundEnvelope, OutboundPriority},
  extension::{Remote, Remoting, RemotingError},
};
use fraktor_utils_core_rs::core::sync::ArcShared;
use tokio::time::{sleep, timeout};

use crate::std::{
  extension_installer::remoting_extension_installer::{RemotingExtensionInstaller, deliver_inbound_envelope},
  tests::test_support::EventHarness,
  transport::tcp::TcpRemoteTransport,
};

struct RecordingBytesActor {
  tx: Sender<Bytes>,
}

impl RecordingBytesActor {
  fn new(tx: Sender<Bytes>) -> Self {
    Self { tx }
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

fn make_transport() -> TcpRemoteTransport {
  TcpRemoteTransport::new("127.0.0.1:0", Vec::new())
}

fn make_transport_with_addresses(addresses: Vec<Address>) -> TcpRemoteTransport {
  TcpRemoteTransport::new("127.0.0.1:0", addresses)
}

fn remote_config() -> RemoteConfig {
  RemoteConfig::new("127.0.0.1")
}

fn make_remote(transport: TcpRemoteTransport) -> (Remote, EventHarness) {
  let harness = EventHarness::new();
  let remote = Remote::new(transport, remote_config(), harness.publisher().clone());
  (remote, harness)
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
async fn extension_installer_holds_a_shared_remote_handle() {
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let installer = RemotingExtensionInstaller::new(make_transport_with_addresses(vec![listen_address]), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should create remote");
  let remote_a = installer.remote().expect("installed remote should be available");
  let remote_b = installer.remote().expect("installed remote should be available");
  assert!(!remote_a.addresses().is_empty(), "install should start the remote");
  assert!(!remote_b.addresses().is_empty(), "second handle should observe the same remote state");
  installer.shutdown_and_join().await.expect("shutdown should join run task");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_config_install_retains_lifecycle_handle() {
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let installer = ArcShared::new(RemotingExtensionInstaller::new(
    make_transport_with_addresses(vec![listen_address]),
    remote_config(),
  ));
  let installers = ExtensionInstallers::default().with_shared_extension_installer(installer.clone());
  let config = std_actor_system_config(TestTickDriver::default()).with_extension_installers(installers);
  let system = ActorSystem::noop_with_config(config).expect("system should install remoting extension");
  let remote = installer.remote().expect("config-installed remote should be available");

  assert!(!remote.addresses().is_empty(), "config install should start remoting");
  installer.shutdown_and_join().await.expect("caller-retained installer should shut down");
  system.terminate().expect("terminate");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_system_termination_shuts_down_remote() {
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let installer = ArcShared::new(RemotingExtensionInstaller::new(
    make_transport_with_addresses(vec![listen_address]),
    remote_config(),
  ));
  let installers = ExtensionInstallers::default().with_shared_extension_installer(installer.clone());
  let config = std_actor_system_config(TestTickDriver::default()).with_extension_installers(installers);
  let system = ActorSystem::noop_with_config(config).expect("system should install remoting extension");
  let remote = installer.remote().expect("config-installed remote should be available");
  assert!(!remote.addresses().is_empty(), "config install should start remoting");

  system.terminate().expect("terminate should trigger remoting shutdown");
  timeout(Duration::from_secs(1), system.when_terminated()).await.expect("system should terminate");
  timeout(Duration::from_secs(1), async {
    while !remote.addresses().is_empty() {
      sleep(Duration::from_millis(10)).await;
    }
  })
  .await
  .expect("remoting should shut down after system termination");
}

#[test]
fn extension_installer_remote_before_install_returns_not_started() {
  let installer = RemotingExtensionInstaller::new(make_transport(), remote_config());

  let error = match installer.remote() {
    | Ok(_) => panic!("remote handle should not exist before install"),
    | Err(error) => error,
  };

  assert_eq!(error, RemotingError::NotStarted);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_double_install_returns_configuration_error() {
  let installer = RemotingExtensionInstaller::new(make_transport(), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("first install should create remote");

  let error = installer.install(harness.system()).expect_err("second install should fail");

  assert_configuration_error(error, "remote extension is already installed");
  installer.shutdown_and_join().await.expect("shutdown should join run task");
}

#[test]
fn inbound_delivery_bridge_sends_bytes_payload_to_local_actor() {
  let config = std_actor_system_config(TestTickDriver::default())
    .with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default());
  let system = ActorSystem::noop_with_config(config).expect("noop actor system should build");
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

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_double_spawn_run_task_returns_already_running() {
  let installer = RemotingExtensionInstaller::new(make_transport(), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should create remote");

  let error = installer.spawn_run_task().expect_err("install-spawned run task should already be running");

  assert_eq!(error, RemotingError::AlreadyRunning);
  installer.shutdown_and_join().await.expect("shutdown should join run task");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_shutdown_and_join_is_idempotent() {
  let installer = RemotingExtensionInstaller::new(make_transport(), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should create remote");

  installer.shutdown_and_join().await.expect("first shutdown should join run task");
  installer.shutdown_and_join().await.expect("second shutdown should be a no-op");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_remote_lifecycle_drives_via_remote_shared_handle() {
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let installer = RemotingExtensionInstaller::new(make_transport_with_addresses(vec![listen_address]), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should wire event publisher");
  let remote = installer.remote().expect("installed remote should be available");
  assert!(!remote.addresses().is_empty());
  installer.shutdown_and_join().await.expect("shutdown should join run task");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_install_wires_listen_event_publisher() {
  let listen_address = Address::new("local-sys", "127.0.0.1", 2551);
  let installer = RemotingExtensionInstaller::new(make_transport_with_addresses(vec![listen_address]), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should wire event publisher");

  let events = harness.events();
  assert_eq!(listen_started_authorities(&events), vec![String::from("local-sys@127.0.0.1:2551")]);
  installer.shutdown_and_join().await.expect("shutdown should join run task");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_start_binds_listener_and_publishes_actual_bound_port() {
  // Given
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let installer = RemotingExtensionInstaller::new(make_transport_with_addresses(vec![listen_address]), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should wire event publisher");
  let remote = installer.remote().expect("installed remote should be available");

  // When
  let advertised_addresses = remote.addresses();

  // Then
  let actual_port = advertised_addresses.first().expect("advertised address").port();
  assert_ne!(actual_port, 0);
  let events = harness.events();
  assert_eq!(listen_started_authorities(&events), vec![alloc::format!("local-sys@127.0.0.1:{actual_port}")]);

  installer.shutdown_and_join().await.expect("shutdown should join run task");
}
