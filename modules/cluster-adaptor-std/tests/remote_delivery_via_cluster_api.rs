//! Cluster-facing remote delivery proof through `ClusterApi::get`.

use std::{format, net::TcpListener, string::String, time::Duration, vec::Vec};

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
use fraktor_cluster_adaptor_std_rs::std::ClusterApi;
use fraktor_cluster_core_rs::core::{
  ClusterExtension, ClusterExtensionConfig, ClusterExtensionInstaller,
  cluster_provider::NoopClusterProvider,
  grain::GrainKey,
  identity::{ClusterIdentity, IdentityLookup, IdentitySetupError, LookupError},
  placement::{ActivatedKind, PlacementDecision, PlacementLocality, PlacementResolution},
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

const SYSTEM_NAME: &str = "cluster-e2e";

struct RecordingBytesActor {
  tx: UnboundedSender<Bytes>,
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

struct StaticIdentityLookup {
  authority: String,
}

impl StaticIdentityLookup {
  fn new(authority: String) -> Self {
    Self { authority }
  }
}

impl IdentityLookup for StaticIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    let pid = format!("{}::{}", self.authority, key.value());
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid,
    })
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

fn remote_extension_parts(
  port: u16,
  uid: u64,
) -> (ArcShared<RemotingExtensionInstaller>, Address, StdRemoteActorRefProviderInstaller) {
  let address = Address::new(SYSTEM_NAME, "127.0.0.1", port);
  let transport = TcpRemoteTransport::new(format!("127.0.0.1:{port}"), vec![address.clone()]);
  let installer = ArcShared::new(RemotingExtensionInstaller::new(transport, RemoteConfig::new("127.0.0.1")));
  let provider_installer = StdRemoteActorRefProviderInstaller::from_remoting_extension_installer(
    UniqueAddress::new(address.clone(), uid),
    installer.clone(),
  );
  (installer, address, provider_installer)
}

fn build_cluster_node(port: u16, uid: u64, remote_authority: String) -> (RemoteNode, ArcShared<ClusterExtension>) {
  let (installer, address, provider_installer) = remote_extension_parts(port, uid);
  let cluster_config = ClusterExtensionConfig::new().with_advertised_address(format!("127.0.0.1:{}", address.port()));
  let cluster_installer = ClusterExtensionInstaller::new(cluster_config, |_event_stream, _block_list, _address| {
    Box::new(NoopClusterProvider::new())
  })
  .with_identity_lookup_factory(move || Box::new(StaticIdentityLookup::new(remote_authority.clone())));
  let extension_installers = ExtensionInstallers::default()
    .with_shared_extension_installer(installer.clone())
    .with_extension_installer(cluster_installer);
  let config = std_actor_system_config(TestTickDriver::default())
    .with_system_name(SYSTEM_NAME)
    .with_extension_installers(extension_installers)
    .with_actor_ref_provider_installer(provider_installer);
  let system = ActorSystem::noop_with_config(config).expect("cluster actor system should build");
  let extension = system.extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  (RemoteNode { system, address }, extension)
}

fn build_remote_node(port: u16, uid: u64) -> RemoteNode {
  let (installer, address, provider_installer) = remote_extension_parts(port, uid);
  let extension_installers = ExtensionInstallers::default().with_shared_extension_installer(installer.clone());
  let config = std_actor_system_config(TestTickDriver::default())
    .with_system_name(SYSTEM_NAME)
    .with_extension_installers(extension_installers)
    .with_actor_ref_provider_installer(provider_installer);
  let system = ActorSystem::noop_with_config(config).expect("remote actor system should build");
  RemoteNode { system, address }
}

fn spawn_recording_actor(system: &ActorSystem, name: &'static str) -> (UnboundedReceiver<Bytes>, ActorPath) {
  let (tx, rx) = mpsc::unbounded_channel();
  let props = Props::from_fn(move || RecordingBytesActor::new(tx.clone()));
  let child = system.actor_of_named(&props, name).expect("recording actor should spawn");
  let path = child.actor_ref().path().expect("recording actor should have a path");
  (rx, path)
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

fn subscribe_lifecycle(system: &ActorSystem) -> (UnboundedReceiver<RemotingLifecycleEvent>, EventStreamSubscription) {
  let (tx, rx) = mpsc::unbounded_channel();
  let subscriber = subscriber_handle(LifecycleRecorder::new(tx));
  let subscription = system.event_stream().subscribe(&subscriber);
  (rx, subscription)
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

#[tokio::test(flavor = "current_thread")]
async fn cluster_api_get_delivers_supported_bytes_payload_to_remote_actor() {
  let remote_port = reserve_port();
  let (cluster_node, cluster_ext) = build_cluster_node(reserve_port(), 1, format!("127.0.0.1:{remote_port}"));
  let remote_node = build_remote_node(remote_port, 2);
  let (mut lifecycle_cluster, _subscription_cluster) = subscribe_lifecycle(&cluster_node.system);
  let (mut lifecycle_remote, _subscription_remote) = subscribe_lifecycle(&remote_node.system);
  let (_rx_cluster, path_cluster) = spawn_recording_actor(&cluster_node.system, "receiver-a");
  let (mut rx_remote, _path_remote) = spawn_recording_actor(&remote_node.system, "receiver-b");

  cluster_ext.start_member().expect("cluster member should start");
  cluster_ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("cluster kind should register");
  let api = ClusterApi::try_from_system(&cluster_node.system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "receiver-b").expect("cluster identity");
  let mut cluster_resolved_ref = api.get(&identity).expect("cluster api should resolve remote actor ref");
  let mut ref_to_cluster = remote_node
    .system
    .resolve_actor_ref(remote_path(&cluster_node.address, &path_cluster))
    .expect("remote node should resolve cluster node actor through configured provider");

  cluster_resolved_ref
    .try_tell(AnyMessage::new(Bytes::from_static(b"warm-cluster-to-remote")))
    .expect("warm cluster send");
  sleep(Duration::from_millis(100)).await;
  ref_to_cluster.try_tell(AnyMessage::new(Bytes::from_static(b"warm-remote-to-cluster"))).expect("warm reverse send");
  wait_until_connected(&mut lifecycle_cluster, &remote_node.address.to_string()).await;
  wait_until_connected(&mut lifecycle_remote, &cluster_node.address.to_string()).await;

  cluster_resolved_ref
    .try_tell(AnyMessage::new(Bytes::from_static(b"cluster-to-remote")))
    .expect("cluster api resolved ref should send through std remote delivery");

  let received = recv_until(&mut rx_remote, Bytes::from_static(b"cluster-to-remote")).await;
  assert_eq!(received, Bytes::from_static(b"cluster-to-remote"));

  cluster_node.shutdown().await;
  remote_node.shutdown().await;
}
