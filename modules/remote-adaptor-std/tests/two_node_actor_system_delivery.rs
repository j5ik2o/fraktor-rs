//! Actor-system level two-node remote delivery proof.

use std::{format, net::TcpListener, time::Duration};

use bytes::Bytes;
use fraktor_actor_adaptor_std_rs::std::{system::std_actor_system_config, tick_driver::TestTickDriver};
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    actor_path::{ActorPath, ActorPathParser},
    error::ActorError,
    extension::ExtensionInstallers,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  event::stream::{
    EventStreamEvent, EventStreamSubscriber, EventStreamSubscription, RemotingLifecycleEvent, subscriber_handle,
  },
  system::ActorSystem,
};
use fraktor_remote_adaptor_std_rs::std::{
  extension_installer::RemotingExtensionInstaller, provider::StdRemoteActorRefProviderInstaller,
  transport::tcp::TcpRemoteTransport,
};
use fraktor_remote_core_rs::core::{
  address::{Address, UniqueAddress},
  config::RemoteConfig,
};
use fraktor_utils_core_rs::core::sync::ArcShared;
use tokio::{
  sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
  time::{Instant, sleep, timeout},
};

const SYSTEM_NAME: &str = "remote-e2e";

struct RecordingBytesActor {
  tx: UnboundedSender<Bytes>,
}

struct LifecycleRecorder {
  tx: UnboundedSender<RemotingLifecycleEvent>,
}

impl LifecycleRecorder {
  fn new(tx: UnboundedSender<RemotingLifecycleEvent>) -> Self {
    Self { tx }
  }
}

impl EventStreamSubscriber for LifecycleRecorder {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::RemotingLifecycle(event) = event {
      let _ = self.tx.send(event.clone());
    }
  }
}

impl RecordingBytesActor {
  fn new(tx: UnboundedSender<Bytes>) -> Self {
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

struct RemoteNode {
  system:  ActorSystem,
  address: Address,
}

impl RemoteNode {
  async fn shutdown(self) {
    self.system.terminate().expect("system should terminate");
    timeout(Duration::from_secs(5), self.system.when_terminated())
      .await
      .expect("system should terminate within timeout");
  }
}

fn reserve_port() -> u16 {
  let listener = TcpListener::bind("127.0.0.1:0").expect("reserve tcp port");
  listener.local_addr().expect("reserved local addr").port()
}

fn build_node(port: u16, uid: u64) -> RemoteNode {
  let address = Address::new(SYSTEM_NAME, "127.0.0.1", port);
  let transport = TcpRemoteTransport::new(format!("127.0.0.1:{port}"), vec![address.clone()]);
  let installer = ArcShared::new(RemotingExtensionInstaller::new(transport, RemoteConfig::new("127.0.0.1")));
  let extension_installers = ExtensionInstallers::default().with_shared_extension_installer(installer.clone());
  let provider_installer = StdRemoteActorRefProviderInstaller::from_remoting_extension_installer(
    UniqueAddress::new(address.clone(), uid),
    installer.clone(),
  );
  let config = std_actor_system_config(TestTickDriver::default())
    .with_system_name(SYSTEM_NAME)
    .with_extension_installers(extension_installers)
    .with_actor_ref_provider_installer(provider_installer);
  let system = ActorSystem::noop_with_config(config).expect("actor system should build");
  RemoteNode { system, address }
}

fn spawn_recording_actor(system: &ActorSystem, name: &'static str) -> (UnboundedReceiver<Bytes>, ActorPath) {
  let (tx, rx) = mpsc::unbounded_channel();
  let props = Props::from_fn(move || RecordingBytesActor::new(tx.clone()));
  let child = system.actor_of_named(&props, name).expect("recording actor should spawn");
  let path = child.actor_ref().path().expect("recording actor should have a path");
  (rx, path)
}

fn subscribe_lifecycle(system: &ActorSystem) -> (UnboundedReceiver<RemotingLifecycleEvent>, EventStreamSubscription) {
  let (tx, rx) = mpsc::unbounded_channel();
  let subscriber = subscriber_handle(LifecycleRecorder::new(tx));
  let subscription = system.event_stream().subscribe(&subscriber);
  (rx, subscription)
}

fn remote_path(address: &Address, local_path: &ActorPath) -> ActorPath {
  ActorPathParser::parse(&format!(
    "fraktor.tcp://{}@{}:{}{}",
    address.system(),
    address.host(),
    address.port(),
    local_path.to_relative_string()
  ))
  .expect("remote actor path should parse")
}

async fn recv_until(rx: &mut UnboundedReceiver<Bytes>, expected: Bytes) -> Bytes {
  let deadline = Instant::now() + Duration::from_secs(5);
  let mut seen = Vec::new();
  loop {
    let now = Instant::now();
    if now >= deadline {
      panic!("expected payload receive timeout; expected={expected:?}; seen={seen:?}");
    }
    match timeout(deadline - now, rx.recv()).await {
      | Ok(Some(bytes)) if bytes == expected => return bytes,
      | Ok(Some(bytes)) => seen.push(bytes),
      | Ok(None) => panic!("recording channel closed before expected payload; expected={expected:?}; seen={seen:?}"),
      | Err(_) => panic!("expected payload receive timeout; expected={expected:?}; seen={seen:?}"),
    }
  }
}

async fn wait_until_connected(rx: &mut UnboundedReceiver<RemotingLifecycleEvent>, expected_authority: &str) {
  timeout(Duration::from_secs(5), async {
    loop {
      match rx.recv().await {
        | Some(RemotingLifecycleEvent::Connected { authority, .. }) if authority == expected_authority => return,
        | Some(_) => {},
        | None => panic!("lifecycle channel closed before Connected"),
      }
    }
  })
  .await
  .expect("Connected lifecycle event timeout");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_actor_system_delivery_sends_supported_bytes_payloads() {
  let node_a = build_node(reserve_port(), 1);
  let node_b = build_node(reserve_port(), 2);
  let (mut lifecycle_a, _subscription_a) = subscribe_lifecycle(&node_a.system);
  let (mut lifecycle_b, _subscription_b) = subscribe_lifecycle(&node_b.system);
  let (mut rx_a, path_a) = spawn_recording_actor(&node_a.system, "receiver-a");
  let (mut rx_b, path_b) = spawn_recording_actor(&node_b.system, "receiver-b");
  let mut local_ref_b = node_b
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node B should resolve its remote-authority path as local");
  let mut local_ref_a = node_a
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &path_a))
    .expect("node A should resolve its remote-authority path as local");
  local_ref_b.try_tell(AnyMessage::new(Bytes::from_static(b"local-b"))).expect("local send to node B");
  local_ref_a.try_tell(AnyMessage::new(Bytes::from_static(b"local-a"))).expect("local send to node A");
  let _ = recv_until(&mut rx_b, Bytes::from_static(b"local-b")).await;
  let _ = recv_until(&mut rx_a, Bytes::from_static(b"local-a")).await;

  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B actor through configured provider");
  let mut ref_to_a = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &path_a))
    .expect("node B should resolve node A actor through configured provider");

  ref_to_b.try_tell(AnyMessage::new(Bytes::from_static(b"warm-b"))).expect("warm send to node B");
  sleep(Duration::from_millis(100)).await;
  ref_to_a.try_tell(AnyMessage::new(Bytes::from_static(b"warm-a"))).expect("warm send to node A");
  wait_until_connected(&mut lifecycle_a, &node_b.address.to_string()).await;
  wait_until_connected(&mut lifecycle_b, &node_a.address.to_string()).await;
  ref_to_b.try_tell(AnyMessage::new(Bytes::from_static(b"to-b"))).expect("send to node B");
  ref_to_a.try_tell(AnyMessage::new(Bytes::from_static(b"to-a"))).expect("send to node A");

  let received_b = recv_until(&mut rx_b, Bytes::from_static(b"to-b")).await;
  let received_a = recv_until(&mut rx_a, Bytes::from_static(b"to-a")).await;
  assert_eq!(received_b, Bytes::from_static(b"to-b"));
  assert_eq!(received_a, Bytes::from_static(b"to-a"));

  node_a.shutdown().await;
  node_b.shutdown().await;
}
