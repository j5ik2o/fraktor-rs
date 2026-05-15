//! Actor-system level two-node remote delivery proof.

use std::{format, net::TcpListener, time::Duration};

use fraktor_actor_adaptor_std_rs::{system::std_actor_system_config, tick_driver::TestTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, Address as ActorAddress, ChildRef, Pid,
    actor_path::{ActorPath, ActorPathParser},
    actor_ref::ActorRef,
    deploy::{Deploy, Deployer, RemoteScope, Scope},
    error::ActorError,
    extension::ExtensionInstallers,
    messaging::{AnyMessage, AnyMessageView},
    props::{DeployableFactoryError, DeployablePropsMetadata, Props},
  },
  event::stream::{
    EventStreamEvent, EventStreamSubscriber, EventStreamSubscription, RemotingLifecycleEvent, subscriber_handle,
  },
  system::{ActorSystem, remote::RemotingConfig},
};
use fraktor_remote_adaptor_std_rs::{
  extension_installer::RemotingExtensionInstaller, provider::StdRemoteActorRefProviderInstaller,
  transport::tcp::TcpRemoteTransport,
};
use fraktor_remote_core_rs::{
  address::{Address, UniqueAddress},
  config::RemoteConfig,
};
use fraktor_utils_core_rs::sync::ArcShared;
use tokio::{
  sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
  time::{Instant, timeout},
};

const SYSTEM_NAME: &str = "remote-e2e";

struct RecordingStringActor {
  tx: UnboundedSender<String>,
}

struct RecordingDeathwatchActor {
  terminated_tx: UnboundedSender<Pid>,
  ack_tx:        UnboundedSender<&'static str>,
}

struct OrderedDeathwatchActor {
  tx: UnboundedSender<OrderedDeathwatchEvent>,
}

struct RemoteDeploymentParentActor {
  child_props:   Props,
  spawned_tx:    UnboundedSender<Result<ActorRef, String>>,
  terminated_tx: UnboundedSender<Pid>,
}

#[derive(Debug, PartialEq, Eq)]
enum OrderedDeathwatchEvent {
  WatchInstalled,
  UserMessage(String),
  Terminated(Pid),
}

struct SpawnRemoteChild;

struct StartRemoteWatch {
  target: ActorRef,
}

struct StopRemoteWatch {
  target: ActorRef,
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

impl RecordingStringActor {
  fn new(tx: UnboundedSender<String>) -> Self {
    Self { tx }
  }
}

impl RecordingDeathwatchActor {
  fn new(terminated_tx: UnboundedSender<Pid>, ack_tx: UnboundedSender<&'static str>) -> Self {
    Self { terminated_tx, ack_tx }
  }
}

impl OrderedDeathwatchActor {
  fn new(tx: UnboundedSender<OrderedDeathwatchEvent>) -> Self {
    Self { tx }
  }
}

impl RemoteDeploymentParentActor {
  fn new(
    child_props: Props,
    spawned_tx: UnboundedSender<Result<ActorRef, String>>,
    terminated_tx: UnboundedSender<Pid>,
  ) -> Self {
    Self { child_props, spawned_tx, terminated_tx }
  }
}

impl StartRemoteWatch {
  fn new(target: ActorRef) -> Self {
    Self { target }
  }
}

impl StopRemoteWatch {
  fn new(target: ActorRef) -> Self {
    Self { target }
  }
}

impl Actor for RecordingStringActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(text) = message.downcast_ref::<String>() {
      self.tx.send(text.clone()).expect("recording channel should be open");
    }
    Ok(())
  }
}

impl Actor for RecordingDeathwatchActor {
  fn receive(&mut self, context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<StartRemoteWatch>() {
      context.watch(&command.target).expect("remote watch should install");
      self.ack_tx.send("watch").expect("ack channel should be open");
    } else if let Some(command) = message.downcast_ref::<StopRemoteWatch>() {
      context.unwatch(&command.target).expect("remote unwatch should install");
      self.ack_tx.send("unwatch").expect("ack channel should be open");
    }
    Ok(())
  }

  fn on_terminated(&mut self, _context: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    self.terminated_tx.send(terminated).expect("termination channel should be open");
    Ok(())
  }
}

impl Actor for OrderedDeathwatchActor {
  fn receive(&mut self, context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<StartRemoteWatch>() {
      context.watch(&command.target).expect("remote watch should install");
      self.tx.send(OrderedDeathwatchEvent::WatchInstalled).expect("ordered channel should be open");
    } else if let Some(text) = message.downcast_ref::<String>() {
      self.tx.send(OrderedDeathwatchEvent::UserMessage(text.clone())).expect("ordered channel should be open");
    }
    Ok(())
  }

  fn on_terminated(&mut self, _context: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    self.tx.send(OrderedDeathwatchEvent::Terminated(terminated)).expect("ordered channel should be open");
    Ok(())
  }
}

impl Actor for RemoteDeploymentParentActor {
  fn receive(&mut self, context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<SpawnRemoteChild>().is_some() {
      let child = match context.spawn_child(&self.child_props) {
        | Ok(child) => child,
        | Err(error) => {
          self
            .spawned_tx
            .send(Err(format!("remote child spawn failed: {error:?}")))
            .map_err(|_| ActorError::recoverable("remote child spawn channel closed"))?;
          return Ok(());
        },
      };
      if let Err(error) = context.watch(child.actor_ref()) {
        self
          .spawned_tx
          .send(Err(format!("remote child watch failed: {error:?}")))
          .map_err(|_| ActorError::recoverable("remote child spawn channel closed"))?;
        return Ok(());
      }
      self
        .spawned_tx
        .send(Ok(child.actor_ref().clone()))
        .map_err(|_| ActorError::recoverable("remote child spawn channel closed"))?;
    }
    Ok(())
  }

  fn on_terminated(&mut self, _context: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    self
      .terminated_tx
      .send(terminated)
      .map_err(|_| ActorError::recoverable("remote child termination channel closed"))?;
    Ok(())
  }
}

struct RemoteNode {
  system:    ActorSystem,
  address:   Address,
  installer: ArcShared<RemotingExtensionInstaller>,
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
  build_node_with_remote_config(port, uid, RemoteConfig::new("127.0.0.1"))
}

fn build_node_with_remote_config(port: u16, uid: u64, remote_config: RemoteConfig) -> RemoteNode {
  build_node_with_remote_config_and_deployer(port, uid, remote_config, Deployer::new())
}

fn build_node_with_deployer(port: u16, uid: u64, deployer: Deployer) -> RemoteNode {
  build_node_with_remote_config_and_deployer(port, uid, RemoteConfig::new("127.0.0.1"), deployer)
}

fn build_node_with_remote_config_and_deployer(
  port: u16,
  uid: u64,
  remote_config: RemoteConfig,
  deployer: Deployer,
) -> RemoteNode {
  let address = Address::new(SYSTEM_NAME, "127.0.0.1", port);
  let transport = TcpRemoteTransport::new(format!("127.0.0.1:{port}"), vec![address.clone()]);
  let installer = ArcShared::new(RemotingExtensionInstaller::new(transport, remote_config));
  let extension_installers = ExtensionInstallers::default().with_shared_extension_installer(installer.clone());
  let provider_installer = StdRemoteActorRefProviderInstaller::from_remoting_extension_installer(
    UniqueAddress::new(address.clone(), uid),
    installer.clone(),
  );
  let config = std_actor_system_config(TestTickDriver::default())
    .with_system_name(SYSTEM_NAME)
    .with_remoting_config(
      RemotingConfig::default().with_canonical_host(address.host()).with_canonical_port(address.port()),
    )
    .with_deployer(deployer)
    .with_extension_installers(extension_installers)
    .with_actor_ref_provider_installer(provider_installer);
  let system = ActorSystem::create_with_noop_guardian(config).expect("actor system should build");
  RemoteNode { system, address, installer }
}

fn spawn_recording_actor(system: &ActorSystem, name: &'static str) -> (UnboundedReceiver<String>, ActorPath) {
  let (rx, _child, path) = spawn_recording_child(system, name);
  (rx, path)
}

fn spawn_recording_child(system: &ActorSystem, name: &'static str) -> (UnboundedReceiver<String>, ChildRef, ActorPath) {
  let (tx, rx) = mpsc::unbounded_channel();
  let props = Props::from_fn(move || RecordingStringActor::new(tx.clone()));
  let child = system.actor_of_named(&props, name).expect("recording actor should spawn");
  let path = child.actor_ref().path().expect("recording actor should have a path");
  (rx, child, path)
}

fn spawn_deathwatch_actor(
  system: &ActorSystem,
  name: &'static str,
) -> (UnboundedReceiver<Pid>, UnboundedReceiver<&'static str>, ActorRef) {
  let (terminated_tx, terminated_rx) = mpsc::unbounded_channel();
  let (ack_tx, ack_rx) = mpsc::unbounded_channel();
  let props = Props::from_fn(move || RecordingDeathwatchActor::new(terminated_tx.clone(), ack_tx.clone()));
  let child = system.actor_of_named(&props, name).expect("death-watch actor should spawn");
  (terminated_rx, ack_rx, child.actor_ref().clone())
}

fn spawn_ordered_deathwatch_actor(
  system: &ActorSystem,
  name: &'static str,
) -> (UnboundedReceiver<OrderedDeathwatchEvent>, ActorRef, ActorPath) {
  let (tx, rx) = mpsc::unbounded_channel();
  let props = Props::from_fn(move || OrderedDeathwatchActor::new(tx.clone()));
  let child = system.actor_of_named(&props, name).expect("ordered death-watch actor should spawn");
  let path = child.actor_ref().path().expect("ordered actor should have a path");
  (rx, child.actor_ref().clone(), path)
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

fn remote_deployer_for_child(child_name: &str, address: &Address) -> Deployer {
  remote_deployer_for_path(&format!("/user/{child_name}"), address)
}

fn remote_deployer_for_child_of(parent_name: &str, child_name: &str, address: &Address) -> Deployer {
  remote_deployer_for_path(&format!("/user/{parent_name}/{child_name}"), address)
}

fn remote_deployer_for_path(path: &str, address: &Address) -> Deployer {
  let mut deployer = Deployer::new();
  let target = ActorAddress::remote(address.system(), address.host(), address.port());
  deployer.register(path, Deploy::new().with_scope(Scope::Remote(RemoteScope::new(target))));
  deployer
}

async fn recv_until(rx: &mut UnboundedReceiver<String>, expected: String) -> String {
  let deadline = Instant::now() + Duration::from_secs(5);
  let mut seen = Vec::new();
  loop {
    let now = Instant::now();
    if now >= deadline {
      panic!("expected payload receive timeout; expected={expected:?}; seen={seen:?}");
    }
    match timeout(deadline - now, rx.recv()).await {
      | Ok(Some(text)) if text == expected => return text,
      | Ok(Some(text)) => seen.push(text),
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

async fn wait_for_ack(rx: &mut UnboundedReceiver<&'static str>, expected: &'static str) {
  timeout(Duration::from_secs(5), async {
    loop {
      match rx.recv().await {
        | Some(actual) if actual == expected => return,
        | Some(_) => {},
        | None => panic!("watch ack channel closed before {expected}"),
      }
    }
  })
  .await
  .expect("watch ack timeout");
}

async fn wait_for_ordered_event(
  rx: &mut UnboundedReceiver<OrderedDeathwatchEvent>,
  expected: OrderedDeathwatchEvent,
) -> OrderedDeathwatchEvent {
  let deadline = Instant::now() + Duration::from_secs(5);
  let mut seen = Vec::new();
  loop {
    let now = Instant::now();
    if now >= deadline {
      panic!("ordered event receive timeout; expected={expected:?}; seen={seen:?}");
    }
    match timeout(deadline - now, rx.recv()).await {
      | Ok(Some(event)) if event == expected => return event,
      | Ok(Some(event)) => seen.push(event),
      | Ok(None) => panic!("ordered channel closed before expected event; expected={expected:?}; seen={seen:?}"),
      | Err(_) => panic!("ordered event receive timeout; expected={expected:?}; seen={seen:?}"),
    }
  }
}

async fn warm_bidirectional_remote_delivery(
  ref_to_b: &mut ActorRef,
  rx_b: &mut UnboundedReceiver<String>,
  ref_to_a: &mut ActorRef,
  rx_a: &mut UnboundedReceiver<String>,
) {
  ref_to_b.try_tell(AnyMessage::new(String::from("warm-b"))).expect("warm send to node B");
  ref_to_a.try_tell(AnyMessage::new(String::from("warm-a"))).expect("warm send to node A");
  let _warm_b = recv_until(rx_b, String::from("warm-b")).await;
  let _warm_a = recv_until(rx_a, String::from("warm-a")).await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_actor_system_delivery_sends_registered_string_payloads() {
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
  local_ref_b.try_tell(AnyMessage::new(String::from("local-b"))).expect("local send to node B");
  local_ref_a.try_tell(AnyMessage::new(String::from("local-a"))).expect("local send to node A");
  let _ = recv_until(&mut rx_b, String::from("local-b")).await;
  let _ = recv_until(&mut rx_a, String::from("local-a")).await;

  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B actor through configured provider");
  let mut ref_to_a = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &path_a))
    .expect("node B should resolve node A actor through configured provider");

  warm_bidirectional_remote_delivery(&mut ref_to_b, &mut rx_b, &mut ref_to_a, &mut rx_a).await;
  wait_until_connected(&mut lifecycle_a, &node_b.address.to_string()).await;
  wait_until_connected(&mut lifecycle_b, &node_a.address.to_string()).await;
  ref_to_b.try_tell(AnyMessage::new(String::from("to-b"))).expect("send to node B");
  ref_to_a.try_tell(AnyMessage::new(String::from("to-a"))).expect("send to node A");

  let received_b = recv_until(&mut rx_b, String::from("to-b")).await;
  let received_a = recv_until(&mut rx_a, String::from("to-a")).await;
  assert_eq!(received_b, String::from("to-b"));
  assert_eq!(received_a, String::from("to-a"));

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_remote_deployment_spawns_actor_and_delivers_user_message() {
  let child_name = "deployed-b";
  let node_b = build_node(reserve_port(), 62);
  let node_a = build_node_with_deployer(reserve_port(), 61, remote_deployer_for_child(child_name, &node_b.address));
  let (mut warm_rx_a, warm_path_a) = spawn_recording_actor(&node_a.system, "deployment-warm-a");
  let (mut warm_rx, warm_path) = spawn_recording_actor(&node_b.system, "deployment-warm-b");
  let mut warm_ref = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &warm_path))
    .expect("node A should resolve warm target on node B");
  let mut warm_ref_a = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &warm_path_a))
    .expect("node B should resolve warm target on node A");
  warm_bidirectional_remote_delivery(&mut warm_ref, &mut warm_rx, &mut warm_ref_a, &mut warm_rx_a).await;
  let (target_tx, mut target_rx) = mpsc::unbounded_channel();
  node_b.system.extended().register_deployable_actor_factory("recording-string", move |payload: AnyMessage| {
    let payload =
      payload.downcast_ref::<String>().ok_or_else(|| DeployableFactoryError::new("payload must be String"))?;
    if payload != "factory-payload" {
      return Err(DeployableFactoryError::new("unexpected factory payload"));
    }
    let tx = target_tx.clone();
    Ok(Props::from_fn(move || RecordingStringActor::new(tx.clone())))
  });
  let (unused_tx, _unused_rx) = mpsc::unbounded_channel();
  let props = Props::from_fn(move || RecordingStringActor::new(unused_tx.clone())).with_deployable_metadata(
    DeployablePropsMetadata::new("recording-string", AnyMessage::new(String::from("factory-payload"))),
  );
  let system_a = node_a.system.clone();
  let child = tokio::task::spawn_blocking(move || system_a.actor_of_named(&props, child_name))
    .await
    .expect("blocking spawn should complete")
    .expect("remote child should spawn");

  let created_path = child.actor_ref().canonical_path().expect("deployed child canonical path");
  let mut local_created_ref =
    node_b.system.resolve_actor_ref(created_path.clone()).expect("target node should resolve deployed child locally");
  local_created_ref.try_tell(AnyMessage::new(String::from("local-deployed"))).expect("local send to deployed child");
  let local_received = recv_until(&mut target_rx, String::from("local-deployed")).await;
  assert_eq!(local_received, String::from("local-deployed"));

  let mut actor_ref = child.actor_ref().clone();
  actor_ref.try_tell(AnyMessage::new(String::from("deployed-message"))).expect("send to deployed child");

  let received = recv_until(&mut target_rx, String::from("deployed-message")).await;
  assert_eq!(received, String::from("deployed-message"));

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_node_remote_deployment_parent_observes_child_termination() {
  let parent_name = "deployment-parent-a";
  let child_name = "watched-deployed-b";
  let node_b = build_node(reserve_port(), 72);
  let node_a = build_node_with_deployer(
    reserve_port(),
    71,
    remote_deployer_for_child_of(parent_name, child_name, &node_b.address),
  );
  let (mut warm_rx_a, warm_path_a) = spawn_recording_actor(&node_a.system, "deployment-watch-warm-a");
  let (mut warm_rx_b, warm_path_b) = spawn_recording_actor(&node_b.system, "deployment-watch-warm-b");
  let mut warm_ref_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &warm_path_b))
    .expect("node A should resolve warm target on node B");
  let mut warm_ref_a = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &warm_path_a))
    .expect("node B should resolve warm target on node A");
  warm_bidirectional_remote_delivery(&mut warm_ref_b, &mut warm_rx_b, &mut warm_ref_a, &mut warm_rx_a).await;

  let (target_tx, mut target_rx) = mpsc::unbounded_channel();
  node_b.system.extended().register_deployable_actor_factory("watched-recording-string", move |payload: AnyMessage| {
    let payload =
      payload.downcast_ref::<String>().ok_or_else(|| DeployableFactoryError::new("payload must be String"))?;
    if payload != "watched-factory-payload" {
      return Err(DeployableFactoryError::new("unexpected factory payload"));
    }
    let tx = target_tx.clone();
    Ok(Props::from_fn(move || RecordingStringActor::new(tx.clone())))
  });
  let (target_parent_tx, _target_parent_rx) = mpsc::unbounded_channel();
  let target_parent_props = Props::from_fn(move || RecordingStringActor::new(target_parent_tx.clone()));
  let _target_parent =
    node_b.system.actor_of_named(&target_parent_props, parent_name).expect("target parent actor should spawn");
  let (unused_tx, _unused_rx) = mpsc::unbounded_channel();
  let child_props = Props::from_fn(move || RecordingStringActor::new(unused_tx.clone()))
    .with_name(child_name)
    .with_deployable_metadata(DeployablePropsMetadata::new(
      "watched-recording-string",
      AnyMessage::new(String::from("watched-factory-payload")),
    ));
  let (spawned_tx, mut spawned_rx) = mpsc::unbounded_channel();
  let (terminated_tx, mut terminated_rx) = mpsc::unbounded_channel();
  let parent_props = Props::from_fn(move || {
    RemoteDeploymentParentActor::new(child_props.clone(), spawned_tx.clone(), terminated_tx.clone())
  });
  let parent = node_a.system.actor_of_named(&parent_props, parent_name).expect("parent actor should spawn");
  let mut parent_ref = parent.actor_ref().clone();

  parent_ref.try_tell(AnyMessage::new(SpawnRemoteChild)).expect("spawn command should enqueue");

  let remote_child = timeout(Duration::from_secs(5), spawned_rx.recv())
    .await
    .expect("remote child spawn notification timeout")
    .expect("spawn channel should stay open")
    .expect("remote child should spawn and install watch");
  let mut remote_child_ref = remote_child.clone();
  remote_child_ref
    .try_tell(AnyMessage::new(String::from("after-parent-watch")))
    .expect("barrier send to remote-deployed child");
  let _after_watch = recv_until(&mut target_rx, String::from("after-parent-watch")).await;
  assert!(terminated_rx.try_recv().is_err(), "parent must not observe termination before target child stops");

  let created_path = remote_child.canonical_path().expect("remote child canonical path");
  let local_child =
    node_b.system.resolve_actor_ref(created_path).expect("target node should resolve remote-deployed child locally");
  node_b.system.stop(&local_child).expect("remote-deployed child should stop locally on target node");

  let terminated = timeout(Duration::from_secs(5), terminated_rx.recv())
    .await
    .expect("remote-deployed child termination notification timeout")
    .expect("termination channel should stay open");
  assert_eq!(terminated, remote_child.pid());

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_remote_actor_termination_notifies_watcher() {
  let node_a = build_node(reserve_port(), 11);
  let node_b = build_node(reserve_port(), 12);
  let (mut rx_a, path_a) = spawn_recording_actor(&node_a.system, "receiver-a-watch");
  let (mut rx_b, target_b, path_b) = spawn_recording_child(&node_b.system, "watched-b");
  let (mut terminated_rx, mut ack_rx, mut watcher_ref) = spawn_deathwatch_actor(&node_a.system, "watcher-a");
  let mut local_ref_b = node_b
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node B should resolve its remote-authority path as local");
  local_ref_b.try_tell(AnyMessage::new(String::from("local-b"))).expect("local send to node B");
  let _ = recv_until(&mut rx_b, String::from("local-b")).await;
  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B target");
  let mut ref_to_a =
    node_b.system.resolve_actor_ref(remote_path(&node_a.address, &path_a)).expect("node B should resolve node A actor");

  warm_bidirectional_remote_delivery(&mut ref_to_b, &mut rx_b, &mut ref_to_a, &mut rx_a).await;
  watcher_ref.try_tell(AnyMessage::new(StartRemoteWatch::new(ref_to_b.clone()))).expect("watch command should enqueue");
  wait_for_ack(&mut ack_rx, "watch").await;
  ref_to_b.try_tell(AnyMessage::new(String::from("after-watch"))).expect("barrier send to node B");
  let _ = recv_until(&mut rx_b, String::from("after-watch")).await;

  target_b.stop().expect("remote target should stop");

  let terminated = timeout(Duration::from_secs(5), terminated_rx.recv())
    .await
    .expect("remote termination notification timeout")
    .expect("termination channel should stay open");
  assert_eq!(terminated, ref_to_b.pid());

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_shutdown_waits_for_flush_ack_before_run_task_finishes() {
  let node_a = build_node_with_remote_config(
    reserve_port(),
    31,
    RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::from_secs(2)),
  );
  let node_b = build_node(reserve_port(), 32);
  let (mut rx_a, path_a) = spawn_recording_actor(&node_a.system, "receiver-a-shutdown-flush");
  let (mut rx_b, path_b) = spawn_recording_actor(&node_b.system, "receiver-b-shutdown-flush");
  let mut local_ref_b = node_b
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node B should resolve its remote-authority path as local");
  local_ref_b.try_tell(AnyMessage::new(String::from("local-b"))).expect("local send to node B");
  let _local_b = recv_until(&mut rx_b, String::from("local-b")).await;
  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B target");
  let mut ref_to_a =
    node_b.system.resolve_actor_ref(remote_path(&node_a.address, &path_a)).expect("node B should resolve node A actor");

  warm_bidirectional_remote_delivery(&mut ref_to_b, &mut rx_b, &mut ref_to_a, &mut rx_a).await;
  ref_to_b.try_tell(AnyMessage::new(String::from("before-shutdown"))).expect("send to node B before shutdown");
  tokio::task::yield_now().await;

  timeout(Duration::from_secs(5), node_a.installer.shutdown_and_join())
    .await
    .expect("shutdown flush should complete within timeout")
    .expect("remote shutdown should succeed");

  let received = recv_until(&mut rx_b, String::from("before-shutdown")).await;
  assert_eq!(received, String::from("before-shutdown"));

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_deathwatch_notification_arrives_after_preceding_remote_message() {
  let node_a = build_node(reserve_port(), 41);
  let node_b = build_node(reserve_port(), 42);
  let (mut ordered_rx, mut ordered_ref, ordered_path) =
    spawn_ordered_deathwatch_actor(&node_a.system, "ordered-watcher-a");
  let (mut rx_b, target_b, path_b) = spawn_recording_child(&node_b.system, "ordered-watched-b");
  let mut local_ref_b = node_b
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node B should resolve its remote-authority path as local");
  local_ref_b.try_tell(AnyMessage::new(String::from("local-b"))).expect("local send to node B");
  let _local_b = recv_until(&mut rx_b, String::from("local-b")).await;
  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B target");
  let mut ref_to_ordered_a = node_b
    .system
    .resolve_actor_ref(remote_path(&node_a.address, &ordered_path))
    .expect("node B should resolve ordered watcher on node A");

  ref_to_b.try_tell(AnyMessage::new(String::from("warm-b"))).expect("warm send to node B");
  ref_to_ordered_a.try_tell(AnyMessage::new(String::from("warm-a"))).expect("warm send to ordered watcher on node A");
  let _warm_b = recv_until(&mut rx_b, String::from("warm-b")).await;
  let _warm_a =
    wait_for_ordered_event(&mut ordered_rx, OrderedDeathwatchEvent::UserMessage(String::from("warm-a"))).await;
  ordered_ref.try_tell(AnyMessage::new(StartRemoteWatch::new(ref_to_b.clone()))).expect("watch command should enqueue");
  let _watch = wait_for_ordered_event(&mut ordered_rx, OrderedDeathwatchEvent::WatchInstalled).await;
  ref_to_b.try_tell(AnyMessage::new(String::from("after-watch"))).expect("barrier send to node B");
  let _after_watch = recv_until(&mut rx_b, String::from("after-watch")).await;

  ref_to_ordered_a
    .try_tell(AnyMessage::new(String::from("before-deathwatch")))
    .expect("send before death-watch notification");
  target_b.stop().expect("remote target should stop");

  let received_message =
    wait_for_ordered_event(&mut ordered_rx, OrderedDeathwatchEvent::UserMessage(String::from("before-deathwatch")))
      .await;
  let terminated = wait_for_ordered_event(&mut ordered_rx, OrderedDeathwatchEvent::Terminated(ref_to_b.pid())).await;
  assert_eq!(received_message, OrderedDeathwatchEvent::UserMessage(String::from("before-deathwatch")));
  assert_eq!(terminated, OrderedDeathwatchEvent::Terminated(ref_to_b.pid()));

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_zero_flush_timeout_does_not_stall_deathwatch_or_shutdown() {
  let node_a = build_node(reserve_port(), 51);
  let node_b = build_node_with_remote_config(
    reserve_port(),
    52,
    RemoteConfig::new("127.0.0.1").with_shutdown_flush_timeout(Duration::ZERO),
  );
  let (mut rx_a, path_a) = spawn_recording_actor(&node_a.system, "receiver-a-timeout");
  let (mut rx_b, target_b, path_b) = spawn_recording_child(&node_b.system, "timeout-watched-b");
  let (mut terminated_rx, mut ack_rx, mut watcher_ref) = spawn_deathwatch_actor(&node_a.system, "timeout-watcher-a");
  let mut local_ref_b = node_b
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node B should resolve its remote-authority path as local");
  local_ref_b.try_tell(AnyMessage::new(String::from("local-b"))).expect("local send to node B");
  let _local_b = recv_until(&mut rx_b, String::from("local-b")).await;
  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B target");
  let mut ref_to_a =
    node_b.system.resolve_actor_ref(remote_path(&node_a.address, &path_a)).expect("node B should resolve node A actor");

  warm_bidirectional_remote_delivery(&mut ref_to_b, &mut rx_b, &mut ref_to_a, &mut rx_a).await;
  watcher_ref.try_tell(AnyMessage::new(StartRemoteWatch::new(ref_to_b.clone()))).expect("watch command should enqueue");
  wait_for_ack(&mut ack_rx, "watch").await;
  ref_to_b.try_tell(AnyMessage::new(String::from("after-watch"))).expect("barrier send to node B");
  let _after_watch = recv_until(&mut rx_b, String::from("after-watch")).await;

  target_b.stop().expect("remote target should stop");

  let terminated = timeout(Duration::from_secs(5), terminated_rx.recv())
    .await
    .expect("remote termination notification timeout")
    .expect("termination channel should stay open");
  assert_eq!(terminated, ref_to_b.pid());
  timeout(Duration::from_secs(5), node_b.installer.shutdown_and_join())
    .await
    .expect("zero-timeout shutdown flush should not stall")
    .expect("remote shutdown should succeed");

  node_a.shutdown().await;
  node_b.shutdown().await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn two_node_remote_unwatch_suppresses_stale_notification() {
  let node_a = build_node(reserve_port(), 21);
  let node_b = build_node(reserve_port(), 22);
  let (mut rx_a, path_a) = spawn_recording_actor(&node_a.system, "receiver-a-unwatch");
  let (mut rx_b, target_b, path_b) = spawn_recording_child(&node_b.system, "unwatched-b");
  let (mut terminated_rx, mut ack_rx, mut watcher_ref) = spawn_deathwatch_actor(&node_a.system, "unwatcher-a");
  let mut local_ref_b = node_b
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node B should resolve its remote-authority path as local");
  local_ref_b.try_tell(AnyMessage::new(String::from("local-b"))).expect("local send to node B");
  let _ = recv_until(&mut rx_b, String::from("local-b")).await;
  let mut ref_to_b = node_a
    .system
    .resolve_actor_ref(remote_path(&node_b.address, &path_b))
    .expect("node A should resolve node B target");
  let mut ref_to_a =
    node_b.system.resolve_actor_ref(remote_path(&node_a.address, &path_a)).expect("node B should resolve node A actor");

  warm_bidirectional_remote_delivery(&mut ref_to_b, &mut rx_b, &mut ref_to_a, &mut rx_a).await;
  watcher_ref.try_tell(AnyMessage::new(StartRemoteWatch::new(ref_to_b.clone()))).expect("watch command should enqueue");
  wait_for_ack(&mut ack_rx, "watch").await;
  ref_to_b.try_tell(AnyMessage::new(String::from("after-watch"))).expect("barrier send to node B");
  let _ = recv_until(&mut rx_b, String::from("after-watch")).await;
  watcher_ref
    .try_tell(AnyMessage::new(StopRemoteWatch::new(ref_to_b.clone())))
    .expect("unwatch command should enqueue");
  wait_for_ack(&mut ack_rx, "unwatch").await;
  ref_to_b.try_tell(AnyMessage::new(String::from("after-unwatch"))).expect("barrier send to node B");
  let _ = recv_until(&mut rx_b, String::from("after-unwatch")).await;

  target_b.stop().expect("remote target should stop");

  assert!(timeout(Duration::from_secs(2), terminated_rx.recv()).await.is_err());

  node_a.shutdown().await;
  node_b.shutdown().await;
}
