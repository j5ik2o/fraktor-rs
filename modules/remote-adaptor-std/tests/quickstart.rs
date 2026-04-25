//! Quickstart integration test for the new `StdRemoting` aggregate.
//!
//! Replaces (conceptually) the legacy `modules/remote/tests/quickstart.rs`
//! that exercised the old god-object `RemotingControlHandle` end-to-end.
//! The legacy test was tightly coupled to the loopback provider, the
//! `RemotingExtensionConfig::with_auto_start(false)` knob, and the
//! `bind_transport_listener_for_test` plumbing — none of which exist in
//! the new design. Phase B's `StdRemoting` instead exposes a small,
//! synchronous lifecycle surface that mirrors the pure
//! `fraktor_remote_core_rs::extension::Remoting` trait.

use std::sync::{Arc, Mutex};

use fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system;
use fraktor_actor_core_rs::core::kernel::{actor::extension::ExtensionInstaller, system::ActorSystem};
use fraktor_remote_adaptor_std_rs::{
  extension_installer::{RemotingExtensionInstaller, StdRemoting},
  tcp_transport::TcpRemoteTransport,
};
use fraktor_remote_core_rs::{
  address::Address,
  association::QuarantineReason,
  extension::{EventPublisher, Remoting, RemotingError},
};

fn make_transport() -> Arc<Mutex<TcpRemoteTransport>> {
  Arc::new(Mutex::new(TcpRemoteTransport::new("127.0.0.1:0", Vec::new())))
}

fn make_event_publisher() -> (ActorSystem, EventPublisher) {
  let system = new_empty_actor_system();
  let publisher = EventPublisher::new(system.downgrade());
  (system, publisher)
}

#[test]
fn quickstart_lifecycle_via_std_remoting_directly() {
  let (_system, publisher) = make_event_publisher();
  let mut remoting = StdRemoting::new(make_transport(), None, publisher);
  assert!(!remoting.lifecycle().is_running());

  remoting.start().expect("start");
  assert!(remoting.lifecycle().is_running());

  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  remoting.quarantine(&address, Some(7), QuarantineReason::new("quickstart")).expect("quarantine while running");

  remoting.shutdown().expect("shutdown");
  assert!(remoting.lifecycle().is_terminated());
}

#[test]
fn quickstart_lifecycle_via_extension_installer() {
  let installer = RemotingExtensionInstaller::new(make_transport());
  let system = new_empty_actor_system();
  installer.install(&system).expect("install remoting extension");
  let remoting = installer.remoting().expect("installed remoting should be available");

  {
    let mut guard = remoting.lock().unwrap();
    guard.start().expect("start through installer-shared handle");
  }
  let snapshot_running = remoting.lock().unwrap().lifecycle().is_running();
  assert!(snapshot_running);

  // Quarantine via the same shared handle.
  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  {
    let mut guard = remoting.lock().unwrap();
    guard
      .quarantine(&address, None, QuarantineReason::new("via installer"))
      .expect("quarantine via installer-shared handle");
  }

  // Shutdown returns NotStarted on a second call.
  {
    let mut guard = remoting.lock().unwrap();
    guard.shutdown().expect("first shutdown");
    let second = guard.shutdown().unwrap_err();
    // Second shutdown is rejected by the lifecycle state machine — the
    // exact variant depends on the closed state-machine semantics.
    assert!(matches!(second, RemotingError::InvalidTransition | RemotingError::NotStarted));
  }
}
