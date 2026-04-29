//! Lifecycle integration tests for `remote-core`'s `Remote`.

use fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system;
use fraktor_actor_core_rs::core::kernel::{actor::extension::ExtensionInstaller, system::ActorSystem};
use fraktor_remote_adaptor_std_rs::std::{
  extension_installer::RemotingExtensionInstaller, transport::tcp::TcpRemoteTransport,
};
use fraktor_remote_core_rs::core::{
  address::Address,
  association::QuarantineReason,
  config::RemoteConfig,
  extension::{EventPublisher, Remote, Remoting, RemotingError},
};

fn make_transport() -> TcpRemoteTransport {
  TcpRemoteTransport::new("127.0.0.1:0", Vec::new())
}

fn make_event_publisher() -> (ActorSystem, EventPublisher) {
  let system = new_empty_actor_system();
  let publisher = EventPublisher::new(system.downgrade());
  (system, publisher)
}

fn remote_config() -> RemoteConfig {
  RemoteConfig::new("127.0.0.1")
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_lifecycle_directly() {
  let (system, publisher) = make_event_publisher();
  let mut remote = Remote::new(make_transport(), remote_config(), publisher);
  assert!(!remote.lifecycle().is_running());

  remote.start().expect("start");
  assert!(remote.lifecycle().is_running());

  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  remote.quarantine(&address, Some(7), QuarantineReason::new("remote-lifecycle")).expect("quarantine while running");

  remote.shutdown().expect("shutdown");
  assert!(remote.lifecycle().is_terminated());
  system.terminate().expect("terminate actor system");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_lifecycle_via_extension_installer() {
  let installer = RemotingExtensionInstaller::new(make_transport(), remote_config());
  let system = new_empty_actor_system();
  installer.install(&system).expect("install remote extension");
  let remote = installer.remote().expect("installed remote should be available");

  {
    remote.with_lock(|remote| remote.start()).expect("start through installer-shared handle");
  }
  let snapshot_running = remote.with_lock(|remote| remote.lifecycle().is_running());
  assert!(snapshot_running);

  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  {
    remote
      .with_lock(|remote| remote.quarantine(&address, None, QuarantineReason::new("via installer")))
      .expect("quarantine via installer-shared handle");
  }

  {
    let second = remote.with_lock(|remote| {
      remote.shutdown().expect("first shutdown");
      remote.shutdown().unwrap_err()
    });
    assert_eq!(second, RemotingError::InvalidTransition);
  }
  system.terminate().expect("terminate actor system");
}
