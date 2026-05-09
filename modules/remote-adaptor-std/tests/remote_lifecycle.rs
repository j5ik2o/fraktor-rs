//! Lifecycle integration tests for `remote-core`'s `Remote`.

use std::{format, net::TcpListener, time::Duration};

use fraktor_actor_adaptor_std_rs::std::{system::new_empty_actor_system, tick_driver::TestTickDriver};
use fraktor_actor_core_kernel_rs::{actor::extension::ExtensionInstallers, system::ActorSystem};
use fraktor_remote_adaptor_std_rs::std::{
  extension_installer::RemotingExtensionInstaller, transport::tcp::TcpRemoteTransport,
};
use fraktor_remote_core_rs::core::{
  address::Address,
  association::QuarantineReason,
  config::RemoteConfig,
  extension::{EventPublisher, Remote},
};
use fraktor_utils_core_rs::core::sync::ArcShared;
use tokio::{net::TcpStream, time::timeout};

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

fn reserve_port() -> u16 {
  let listener = TcpListener::bind("127.0.0.1:0").expect("reserve tcp port");
  listener.local_addr().expect("reserved local addr").port()
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_lifecycle_directly() {
  let (system, publisher) = make_event_publisher();
  let mut remote = Remote::new(make_transport(), remote_config(), publisher);
  assert!(!remote.lifecycle().is_running());

  remote.start().expect("start");
  assert!(remote.lifecycle().is_running());

  let address = Address::new("remote-sys", "10.0.0.1", 2552);
  remote.quarantine(&address, Some(7), QuarantineReason::new("remote-lifecycle"), 1).expect("quarantine while running");

  remote.shutdown().expect("shutdown");
  assert!(remote.lifecycle().is_terminated());
  system.terminate().expect("terminate actor system");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn remote_lifecycle_via_extension_installer() {
  let port = reserve_port();
  let address = Address::new("local-sys", "127.0.0.1", port);
  let transport = TcpRemoteTransport::new(format!("127.0.0.1:{port}"), vec![address]);
  let installer = ArcShared::new(RemotingExtensionInstaller::new(transport, remote_config()));
  let installers = ExtensionInstallers::default().with_shared_extension_installer(installer);
  let config = fraktor_actor_adaptor_std_rs::std::system::std_actor_system_config(TestTickDriver::default())
    .with_extension_installers(installers);
  let system = ActorSystem::create_with_noop_guardian(config).expect("system should install and start remoting");

  timeout(Duration::from_secs(5), TcpStream::connect(("127.0.0.1", port)))
    .await
    .expect("remote listener should accept connections")
    .expect("remote listener should be reachable");

  system.terminate().expect("terminate actor system");
  timeout(Duration::from_secs(5), system.when_terminated()).await.expect("system should terminate within timeout");
}
