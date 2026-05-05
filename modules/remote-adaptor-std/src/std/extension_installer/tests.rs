use fraktor_actor_core_rs::core::kernel::{
  actor::extension::ExtensionInstaller,
  event::stream::{CorrelationId, EventStreamEvent, RemotingLifecycleEvent},
  system::ActorSystemBuildError,
};
use fraktor_remote_core_rs::core::{
  address::Address,
  association::QuarantineReason,
  config::RemoteConfig,
  extension::{Remote, Remoting, RemotingError},
};

use crate::std::{
  extension_installer::remoting_extension_installer::RemotingExtensionInstaller, tests::test_support::EventHarness,
  transport::tcp::TcpRemoteTransport,
};

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
  remote_a.start().expect("start through first shared handle");
  assert!(!remote_b.addresses().is_empty(), "second handle should observe the same remote state");
  remote_b.shutdown().expect("shutdown through second shared handle");
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

#[test]
fn extension_installer_double_install_returns_configuration_error() {
  let installer = RemotingExtensionInstaller::new(make_transport(), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("first install should create remote");

  let error = installer.install(harness.system()).expect_err("second install should fail");

  assert_configuration_error(error, "remote extension is already installed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_double_spawn_run_task_returns_already_running() {
  let installer = RemotingExtensionInstaller::new(make_transport(), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should create remote");
  let remote = installer.remote().expect("installed remote should be available");
  remote.start().expect("start through installer-shared handle");
  installer.spawn_run_task().expect("first run task should spawn");

  let error = installer.spawn_run_task().expect_err("second run task should fail");

  assert_eq!(error, RemotingError::AlreadyRunning);
  installer.shutdown_and_join().await.expect("shutdown should join first run task");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_spawn_run_task_after_shutdown_join_reuses_receiver() {
  let installer = RemotingExtensionInstaller::new(make_transport(), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should create remote");
  let remote = installer.remote().expect("installed remote should be available");
  remote.start().expect("start through installer-shared handle");
  installer.spawn_run_task().expect("first run task should spawn");
  installer.shutdown_and_join().await.expect("shutdown should restore run receiver");

  installer.spawn_run_task().expect("run task should spawn again after join");
  installer.shutdown_and_join().await.expect("second shutdown should join second run task");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_remote_lifecycle_drives_via_remote_shared_handle() {
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let installer = RemotingExtensionInstaller::new(make_transport_with_addresses(vec![listen_address]), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should wire event publisher");
  let remote = installer.remote().expect("installed remote should be available");
  remote.start().expect("start through installer-shared handle");
  assert!(!remote.addresses().is_empty());
  remote.shutdown().expect("shutdown through installer-shared handle");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_install_wires_listen_event_publisher() {
  let listen_address = Address::new("local-sys", "127.0.0.1", 2551);
  let installer = RemotingExtensionInstaller::new(make_transport_with_addresses(vec![listen_address]), remote_config());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should wire event publisher");

  {
    let remote = installer.remote().expect("installed remote should be available");
    remote.start().expect("start should publish through installed publisher");
  }

  let events = harness.events();
  assert_eq!(listen_started_authorities(&events), vec![String::from("local-sys@127.0.0.1:2551")]);
  let remote = installer.remote().expect("installed remote should be available");
  remote.shutdown().expect("shutdown after publisher check");
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
  remote.start().expect("start through installer-shared handle");
  let advertised_addresses = remote.addresses();

  // Then
  let actual_port = advertised_addresses.first().expect("advertised address").port();
  assert_ne!(actual_port, 0);
  let events = harness.events();
  assert_eq!(listen_started_authorities(&events), vec![alloc::format!("local-sys@127.0.0.1:{actual_port}")]);

  remote.shutdown().expect("shutdown after bound port check");
}
