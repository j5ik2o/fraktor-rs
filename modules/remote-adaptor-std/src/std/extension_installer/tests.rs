use fraktor_actor_core_rs::core::kernel::{
  actor::extension::ExtensionInstaller,
  event::stream::{CorrelationId, EventStreamEvent, RemotingLifecycleEvent},
  system::ActorSystemBuildError,
};
use fraktor_remote_core_rs::core::{
  address::Address,
  association::QuarantineReason,
  extension::{Remoting, RemotingError},
};
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};

use crate::std::{
  extension_installer::{base::StdRemoting, remoting_extension_installer::RemotingExtensionInstaller},
  tcp_transport::TcpRemoteTransport,
  tests::test_support::EventHarness,
};

fn make_transport() -> SharedLock<TcpRemoteTransport> {
  SharedLock::new_with_driver::<DefaultMutex<_>>(TcpRemoteTransport::new("127.0.0.1:0", Vec::new()))
}

fn make_transport_with_addresses(addresses: Vec<Address>) -> SharedLock<TcpRemoteTransport> {
  SharedLock::new_with_driver::<DefaultMutex<_>>(TcpRemoteTransport::new("127.0.0.1:0", addresses))
}

fn make_remoting(transport: SharedLock<TcpRemoteTransport>) -> (StdRemoting, EventHarness) {
  let harness = EventHarness::new();
  let remoting = StdRemoting::new(transport, None, harness.publisher().clone());
  (remoting, harness)
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
async fn std_remoting_lifecycle_starts_and_shuts_down() {
  let (mut remoting, _harness) = make_remoting(make_transport());
  assert!(!remoting.lifecycle().is_running());

  remoting.start().expect("start should succeed from Pending");
  assert!(remoting.lifecycle().is_running());

  remoting.shutdown().expect("shutdown should succeed from Running");
  assert!(remoting.lifecycle().is_terminated());
}

#[test]
fn std_remoting_shutdown_from_pending_terminates_without_error() {
  let (mut remoting, _harness) = make_remoting(make_transport());

  remoting.shutdown().expect("shutdown should succeed from Pending");

  assert!(remoting.lifecycle().is_terminated());
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn std_remoting_double_start_returns_already_running() {
  let (mut remoting, _harness) = make_remoting(make_transport());
  remoting.start().expect("first start");
  let err = remoting.start().unwrap_err();
  assert_eq!(err, RemotingError::AlreadyRunning);
  remoting.shutdown().expect("shutdown after double-start check");
}

#[test]
fn std_remoting_quarantine_requires_running_state() {
  let (mut remoting, _harness) = make_remoting(make_transport());
  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  let err = remoting.quarantine(&address, Some(1), QuarantineReason::new("not started")).unwrap_err();
  assert_eq!(err, RemotingError::NotStarted);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn std_remoting_quarantine_succeeds_while_running() {
  let (mut remoting, _harness) = make_remoting(make_transport());
  remoting.start().expect("start");
  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  remoting.quarantine(&address, Some(1), QuarantineReason::new("test")).expect("quarantine while running");
  remoting.shutdown().expect("shutdown after quarantine");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn std_remoting_start_snapshots_advertised_addresses() {
  let addresses = vec![Address::new("local-sys", "127.0.0.1", 2551), Address::new("local-sys", "127.0.0.2", 2552)];
  let (mut remoting, _harness) = make_remoting(make_transport_with_addresses(addresses.clone()));

  remoting.start().expect("start should snapshot advertised addresses");

  assert_eq!(remoting.addresses(), addresses.as_slice());
  remoting.shutdown().expect("shutdown after snapshot check");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn std_remoting_start_publishes_listen_started_for_each_advertised_address() {
  let addresses = vec![Address::new("local-sys", "127.0.0.1", 2551), Address::new("local-sys", "127.0.0.2", 2552)];
  let (mut remoting, harness) = make_remoting(make_transport_with_addresses(addresses));

  remoting.start().expect("start should publish listen events");

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
  remoting.shutdown().expect("shutdown after listen event check");
}

#[test]
fn extension_installer_holds_a_shared_remoting_handle() {
  let installer = RemotingExtensionInstaller::new(make_transport());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should create remoting");
  let remoting_a = installer.remoting().expect("installed remoting should be available");
  let remoting_b = installer.remoting().expect("installed remoting should be available");
  assert!(SharedLock::ptr_eq(&remoting_a, &remoting_b), "installer should hand out the same shared lock");
}

#[test]
fn extension_installer_remoting_before_install_returns_configuration_error() {
  let installer = RemotingExtensionInstaller::new(make_transport());

  let error = match installer.remoting() {
    | Ok(_) => panic!("remoting handle should not exist before install"),
    | Err(error) => error,
  };

  assert_configuration_error(error, "remoting extension is not installed");
}

#[test]
fn extension_installer_double_install_returns_configuration_error() {
  let installer = RemotingExtensionInstaller::new(make_transport());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("first install should create remoting");

  let error = installer.install(harness.system()).expect_err("second install should fail");

  assert_configuration_error(error, "remoting extension is already installed");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_remoting_lifecycle_drives_via_shared_lock() {
  let installer = RemotingExtensionInstaller::new(make_transport());
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should wire event publisher");
  let remoting = installer.remoting().expect("installed remoting should be available");
  remoting.with_lock(|remoting| remoting.start()).expect("start through installer-shared handle");
  let snapshot_running = remoting.with_lock(|remoting| remoting.lifecycle().is_running());
  assert!(snapshot_running);
  remoting.with_lock(|remoting| remoting.shutdown()).expect("shutdown through installer-shared handle");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_install_wires_listen_event_publisher() {
  let listen_address = Address::new("local-sys", "127.0.0.1", 2551);
  let installer = RemotingExtensionInstaller::new(make_transport_with_addresses(vec![listen_address]));
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should wire event publisher");

  {
    let remoting = installer.remoting().expect("installed remoting should be available");
    remoting.with_lock(|remoting| remoting.start()).expect("start should publish through installed publisher");
  }

  let events = harness.events();
  assert_eq!(listen_started_authorities(&events), vec![String::from("local-sys@127.0.0.1:2551")]);
  let remoting = installer.remoting().expect("installed remoting should be available");
  remoting.with_lock(|remoting| remoting.shutdown()).expect("shutdown after publisher check");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn extension_installer_start_binds_listener_and_publishes_actual_bound_port() {
  // Given
  let listen_address = Address::new("local-sys", "127.0.0.1", 0);
  let installer = RemotingExtensionInstaller::new(make_transport_with_addresses(vec![listen_address]));
  let harness = EventHarness::new();
  installer.install(harness.system()).expect("install should wire event publisher");
  let remoting = installer.remoting().expect("installed remoting should be available");

  // When
  let advertised_addresses = remoting.with_lock(|remoting| {
    remoting.start().expect("start through installer-shared handle");
    remoting.addresses().to_vec()
  });

  // Then
  let actual_port = advertised_addresses.first().expect("advertised address").port();
  assert_ne!(actual_port, 0);
  let events = harness.events();
  assert_eq!(listen_started_authorities(&events), vec![alloc::format!("local-sys@127.0.0.1:{actual_port}")]);

  remoting.with_lock(|remoting| remoting.shutdown()).expect("shutdown after bound port check");
}
