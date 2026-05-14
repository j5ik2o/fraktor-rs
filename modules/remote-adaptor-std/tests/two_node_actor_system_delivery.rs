//! Actor-system level two-node remote delivery proof.

use std::{format, net::TcpListener, time::Duration};

use fraktor_actor_adaptor_std_rs::{system::std_actor_system_config, tick_driver::TestTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, ChildRef, Pid,
    actor_path::{ActorPath, ActorPathParser},
    actor_ref::ActorRef,
    error::ActorError,
    extension::ExtensionInstallers,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
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
  time::{Instant, sleep, timeout},
};

const SYSTEM_NAME: &str = "remote-e2e";

struct RecordingStringActor {
  tx: UnboundedSender<String>,
}

struct RecordingDeathwatchActor {
  terminated_tx: UnboundedSender<Pid>,
  ack_tx:        UnboundedSender<&'static str>,
}

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
    .with_remoting_config(
      RemotingConfig::default().with_canonical_host(address.host()).with_canonical_port(address.port()),
    )
    .with_extension_installers(extension_installers)
    .with_actor_ref_provider_installer(provider_installer);
  let system = ActorSystem::create_with_noop_guardian(config).expect("actor system should build");
  RemoteNode { system, address }
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

  ref_to_b.try_tell(AnyMessage::new(String::from("warm-b"))).expect("warm send to node B");
  sleep(Duration::from_millis(100)).await;
  ref_to_a.try_tell(AnyMessage::new(String::from("warm-a"))).expect("warm send to node A");
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

  ref_to_b.try_tell(AnyMessage::new(String::from("warm-b"))).expect("warm send to node B");
  sleep(Duration::from_millis(100)).await;
  ref_to_a.try_tell(AnyMessage::new(String::from("warm-a"))).expect("warm send to node A");
  let _ = recv_until(&mut rx_b, String::from("warm-b")).await;
  let _ = recv_until(&mut rx_a, String::from("warm-a")).await;
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

  ref_to_b.try_tell(AnyMessage::new(String::from("warm-b"))).expect("warm send to node B");
  sleep(Duration::from_millis(100)).await;
  ref_to_a.try_tell(AnyMessage::new(String::from("warm-a"))).expect("warm send to node A");
  let _ = recv_until(&mut rx_b, String::from("warm-b")).await;
  let _ = recv_until(&mut rx_a, String::from("warm-a")).await;
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

  assert!(timeout(Duration::from_millis(500), terminated_rx.recv()).await.is_err());

  node_a.shutdown().await;
  node_b.shutdown().await;
}
