use std::sync::{Arc, Mutex};

use fraktor_remote_core_rs::{
  address::Address,
  association::QuarantineReason,
  extension::{Remoting, RemotingError},
};

use crate::{
  extension_installer::{base::StdRemoting, remoting_extension_installer::RemotingExtensionInstaller},
  tcp_transport::TcpRemoteTransport,
};

fn make_transport() -> Arc<Mutex<TcpRemoteTransport>> {
  Arc::new(Mutex::new(TcpRemoteTransport::new("127.0.0.1:0", Vec::new())))
}

#[test]
fn std_remoting_lifecycle_starts_and_shuts_down() {
  let mut remoting = StdRemoting::new(make_transport(), None);
  assert!(!remoting.lifecycle().is_running());

  remoting.start().expect("start should succeed from Pending");
  assert!(remoting.lifecycle().is_running());

  remoting.shutdown().expect("shutdown should succeed from Running");
  assert!(remoting.lifecycle().is_terminated());
}

#[test]
fn std_remoting_double_start_returns_already_running() {
  let mut remoting = StdRemoting::new(make_transport(), None);
  remoting.start().expect("first start");
  let err = remoting.start().unwrap_err();
  assert_eq!(err, RemotingError::AlreadyRunning);
}

#[test]
fn std_remoting_quarantine_requires_running_state() {
  let mut remoting = StdRemoting::new(make_transport(), None);
  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  let err = remoting.quarantine(&address, Some(1), QuarantineReason::new("not started")).unwrap_err();
  assert_eq!(err, RemotingError::NotStarted);
}

#[test]
fn std_remoting_quarantine_succeeds_while_running() {
  let mut remoting = StdRemoting::new(make_transport(), None);
  remoting.start().expect("start");
  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  remoting.quarantine(&address, Some(1), QuarantineReason::new("test")).expect("quarantine while running");
}

#[test]
fn extension_installer_holds_a_shared_remoting_handle() {
  let installer = RemotingExtensionInstaller::new(make_transport());
  let remoting_a = installer.remoting();
  let remoting_b = installer.remoting();
  assert!(Arc::ptr_eq(&remoting_a, &remoting_b), "installer should hand out the same Arc");
}

#[test]
fn extension_installer_remoting_lifecycle_drives_via_arc() {
  let installer = RemotingExtensionInstaller::new(make_transport());
  let remoting = installer.remoting();
  {
    let mut guard = remoting.lock().unwrap();
    guard.start().expect("start through installer-shared handle");
  }
  let snapshot_running = remoting.lock().unwrap().lifecycle().is_running();
  assert!(snapshot_running);
}
