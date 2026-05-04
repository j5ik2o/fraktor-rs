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
  extension::{EventPublisher, Remote, Remoting},
};

fn make_transport() -> TcpRemoteTransport {
  TcpRemoteTransport::new("127.0.0.1:0", vec![Address::new("local-sys", "127.0.0.1", 0)])
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

  remote.start().expect("start through installer-shared handle");
  assert!(!remote.addresses().is_empty());

  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  remote
    .quarantine(&address, None, QuarantineReason::new("via installer"))
    .expect("quarantine via installer-shared handle");

  remote.shutdown().expect("first shutdown");
  remote.shutdown().expect("shared shutdown is idempotent");
  system.terminate().expect("terminate actor system");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_lifecycle_installer_spawn_run_task_and_shutdown_join() {
  let installer = RemotingExtensionInstaller::new(make_transport(), remote_config());
  let system = new_empty_actor_system();
  installer.install(&system).expect("install remote extension");
  let remote = installer.remote().expect("installed remote should be available");
  remote.start().expect("start through installer-shared handle");
  installer.spawn_run_task().expect("spawn remote run task");

  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  remote
    .quarantine(&address, None, QuarantineReason::new("parallel command while run task is pending"))
    .expect("quarantine via shared handle while run task owns event loop");

  installer.shutdown_and_join().await.expect("shutdown wake should join run task");
  system.terminate().expect("terminate actor system");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_lifecycle_standalone_shutdown_then_wake_joins_run_task() {
  let installer = RemotingExtensionInstaller::new(make_transport(), remote_config());
  let system = new_empty_actor_system();
  installer.install(&system).expect("install remote extension");
  let remote = installer.remote().expect("installed remote should be available");
  remote.start().expect("start through installer-shared handle");
  installer.spawn_run_task().expect("spawn remote run task");

  remote.shutdown().expect("standalone shutdown should update lifecycle");
  installer.shutdown_and_join().await.expect("shutdown wake should let run task observe termination");
  system.terminate().expect("terminate actor system");
}
