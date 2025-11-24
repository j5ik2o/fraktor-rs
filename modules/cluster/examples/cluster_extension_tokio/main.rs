#![allow(clippy::print_stdout)]

//! Cluster extension quickstart (Tokio + fraktor.tcp)
//! 実行例:
//! `cargo run -p fraktor-cluster-rs --example cluster_extension_tokio --features std`

#[cfg(not(feature = "std"))]
compile_error!("cluster_extension_tokio example requires `--features std`");

use std::{
  sync::{Arc, Mutex},
  thread,
  time::Duration,
};

use anyhow::{Result, anyhow};
use fraktor_actor_rs::{
  core::{
    error::ActorError, extension::ExtensionInstallers, serialization::SerializationExtensionInstaller,
    system::RemotingConfig,
  },
  std::{
    actor_prim::{Actor, ActorContext, ActorRef},
    dispatcher::{DispatchExecutorAdapter, DispatcherConfig, dispatch_executor::TokioExecutor},
    event_stream::{EventStreamEvent, EventStreamSubscriber},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick::TickDriverConfig,
    system::{ActorSystem, ActorSystemConfig},
  },
};
use fraktor_cluster_rs::{
  core::{
    ActivatedKind, ClusterEvent, ClusterExtensionConfig, ClusterExtensionId, ClusterPubSub, ClusterTopology, Gossiper,
    IdentityLookup, IdentitySetupError,
  },
  std::noop_cluster_provider::NoopClusterProvider,
};
use fraktor_remote_rs::core::{
  BlockListProvider, RemotingExtensionConfig, RemotingExtensionId, RemotingExtensionInstaller,
  TokioActorRefProviderInstaller, TokioTransportConfig, default_loopback_setup,
};
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};
use tokio::sync::oneshot;

const HOST: &str = "127.0.0.1";
const NODE_A_PORT: u16 = 26050;
const NODE_B_PORT: u16 = 26051;
const HUB_NAME: &str = "grain-hub";
const GRAIN_KIND: &str = "grain";
const SAMPLE_KEY: &str = "user:va-1";

#[tokio::main]
async fn main() -> Result<()> {
  // 返信待機チャネル（ノードB→ノードA→ノードB）
  let (reply_tx, reply_rx) = oneshot::channel::<String>();
  let shared_reply = Arc::new(Mutex::new(Some(reply_tx)));

  // ノードA: 受信・Grain 起動側
  let node_a = build_cluster_node("cluster-node-a", NODE_A_PORT, None)?;
  // ノードB: 送信・返信受信側
  let node_b = build_cluster_node("cluster-node-b", NODE_B_PORT, Some(shared_reply.clone()))?;

  // Kind を登録し、クラスタをメンバーモードで起動
  node_a
    .cluster
    .setup_member_kinds(vec![ActivatedKind::new(GRAIN_KIND)])
    .map_err(|e| anyhow!("identity setup (node a): {e:?}"))?;
  node_b
    .cluster
    .setup_member_kinds(vec![ActivatedKind::new(GRAIN_KIND)])
    .map_err(|e| anyhow!("identity setup (node b): {e:?}"))?;
  node_a.cluster.start_member().map_err(|e| anyhow!("start_member node a: {e:?}"))?;
  node_b.cluster.start_member().map_err(|e| anyhow!("start_member node b: {e:?}"))?;

  // トポロジを共有（シンプルな2ノード join を模擬）
  let topology = ClusterTopology::new(1, vec![node_b.advertised.clone()], Vec::new());
  node_a.cluster.on_topology(&topology);
  node_b.cluster.on_topology(&ClusterTopology::new(2, vec![node_a.advertised.clone()], Vec::new()));

  // Grain 呼び出し（ノードBからノードAへリモート送信）
  node_b
    .system
    .user_guardian_ref()
    .tell(AnyMessage::new(StartGrainCall {
      target: node_a.system.user_guardian_ref(),
      key:    SAMPLE_KEY.to_string(),
      body:   "hello cluster over tokio tcp".to_string(),
    }))
    .map_err(|e| anyhow!("start grain send failed: {e:?}"))?;

  // 返信受信
  let reply = tokio::time::timeout(Duration::from_secs(5), reply_rx)
    .await
    .map_err(|_| anyhow!("timeout waiting reply"))?
    .map_err(|_| anyhow!("reply channel dropped"))?;
  println!("[ok] grain reply: {reply}");

  // シャットダウン
  node_b.cluster.shutdown(true).map_err(|e| anyhow!("shutdown node b: {e:?}"))?;
  node_a.cluster.shutdown(true).map_err(|e| anyhow!("shutdown node a: {e:?}"))?;
  drop(node_b.system);
  drop(node_a.system);
  thread::sleep(Duration::from_millis(200));
  Ok(())
}

struct ClusterNode {
  system:     ActorSystem,
  cluster:    ArcShared<fraktor_cluster_rs::core::ClusterExtensionGeneric<StdToolbox>>,
  advertised: String,
}

fn build_cluster_node(
  system_name: &str,
  port: u16,
  responder: Option<Arc<Mutex<Option<oneshot::Sender<String>>>>>,
) -> Result<ClusterNode> {
  let tokio_handle = tokio::runtime::Handle::current();
  let tokio_executor = TokioExecutor::new(tokio_handle);
  let executor_adapter = DispatchExecutorAdapter::new(ArcShared::new(tokio_executor));
  let default_dispatcher = DispatcherConfig::from_executor(ArcShared::new(executor_adapter));

  let remoting_config = RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp");
  let system_config = ActorSystemConfig::default()
    .with_system_name(system_name.to_string())
    .with_tick_driver(TickDriverConfig::tokio_quickstart())
    .with_default_dispatcher(default_dispatcher)
    .with_actor_ref_provider_installer(TokioActorRefProviderInstaller::from_config(TokioTransportConfig::default()))
    .with_remoting_config(RemotingConfig::default().with_canonical_host(HOST).with_canonical_port(port))
    .with_extension_installers(
      ExtensionInstallers::default()
        .with_extension_installer(SerializationExtensionInstaller::new(default_loopback_setup()))
        .with_extension_installer(RemotingExtensionInstaller::new(remoting_config.clone())),
    );

  let guardian = Props::from_fn(GrainHub::new).with_name(HUB_NAME);
  let system = ActorSystem::new_with_config(&guardian, &system_config)
    .map_err(|e| anyhow!("actor system build failed ({system_name}): {e:?}"))?;

  let remoting_id = RemotingExtensionId::<StdToolbox>::new(remoting_config);
  let _remoting = system.extended().register_extension(&remoting_id);

  if let Some(tx) = responder {
    system
      .user_guardian_ref()
      .tell(AnyMessage::new(RegisterResponder { tx }))
      .map_err(|e| anyhow!("register responder failed: {e:?}"))?;
  }

  let event_subscriber: ArcShared<dyn EventStreamSubscriber> =
    ArcShared::new(ClusterEventPrinter::new(system_name.to_string()));
  let _subscription = system.subscribe_event_stream(&event_subscriber);

  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockListProvider);
  let provider: ArcShared<dyn fraktor_cluster_rs::core::ClusterProvider> = ArcShared::new(NoopClusterProvider::new());
  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(LoggingGossiper::new(system_name));
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(LoggingPubSub::new(system_name));
  let identity: ArcShared<dyn IdentityLookup> = ArcShared::new(LoggingIdentityLookup::new(system_name));

  let advertised = format!("{HOST}:{port}");
  let cluster_config =
    ClusterExtensionConfig::default().with_advertised_address(advertised.clone()).with_metrics_enabled(true);
  let cluster_id =
    ClusterExtensionId::<StdToolbox>::new(cluster_config, provider, block_list, gossiper, pubsub, identity);
  let cluster = system.extended().register_extension(&cluster_id);

  Ok(ClusterNode { system, cluster, advertised })
}

// === EventStream subscriber ===

struct ClusterEventPrinter {
  node: String,
}

impl ClusterEventPrinter {
  fn new(node: String) -> Self {
    Self { node }
  }
}

impl EventStreamSubscriber for ClusterEventPrinter {
  fn on_event(&self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { name, payload } = event {
      if name == "cluster" {
        let view = payload.as_view();
        if let Some(cluster_event) = view.downcast_ref::<ClusterEvent>() {
          println!("[cluster][{}] {:?}", self.node, cluster_event);
        }
      }
    }
  }
}

// === Cluster dependencies (no-ops forサンプル) ===

#[derive(Default)]
struct EmptyBlockListProvider;

impl BlockListProvider for EmptyBlockListProvider {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

struct LoggingGossiper {
  label:   String,
  started: Mutex<bool>,
}

impl LoggingGossiper {
  fn new(label: impl Into<String>) -> Self {
    Self { label: label.into(), started: Mutex::new(false) }
  }
}

impl Gossiper for LoggingGossiper {
  fn start(&self) -> Result<(), &'static str> {
    let mut guard = self.started.lock().expect("gossiper lock");
    if !*guard {
      println!("[gossip][{}] start", self.label);
      *guard = true;
    }
    Ok(())
  }

  fn stop(&self) -> Result<(), &'static str> {
    let mut guard = self.started.lock().expect("gossiper lock");
    if *guard {
      println!("[gossip][{}] stop", self.label);
      *guard = false;
    }
    Ok(())
  }
}

struct LoggingPubSub {
  label:   String,
  started: Mutex<bool>,
}

impl LoggingPubSub {
  fn new(label: impl Into<String>) -> Self {
    Self { label: label.into(), started: Mutex::new(false) }
  }
}

impl ClusterPubSub for LoggingPubSub {
  fn start(&self) -> Result<(), fraktor_cluster_rs::core::PubSubError> {
    let mut guard = self.started.lock().expect("pubsub lock");
    if !*guard {
      println!("[pubsub][{}] start", self.label);
      *guard = true;
    }
    Ok(())
  }

  fn stop(&self) -> Result<(), fraktor_cluster_rs::core::PubSubError> {
    let mut guard = self.started.lock().expect("pubsub lock");
    if *guard {
      println!("[pubsub][{}] stop", self.label);
      *guard = false;
    }
    Ok(())
  }
}

struct LoggingIdentityLookup {
  label: String,
}

impl LoggingIdentityLookup {
  fn new(label: impl Into<String>) -> Self {
    Self { label: label.into() }
  }
}

impl IdentityLookup for LoggingIdentityLookup {
  fn setup_member(&self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    let names: Vec<_> = kinds.iter().map(|k| k.name().to_string()).collect();
    println!("[identity][{}] member kinds: {:?}", self.label, names);
    Ok(())
  }

  fn setup_client(&self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    let names: Vec<_> = kinds.iter().map(|k| k.name().to_string()).collect();
    println!("[identity][{}] client kinds: {:?}", self.label, names);
    Ok(())
  }
}

// === Actors ===

struct GrainHub {
  responder: Option<Arc<Mutex<Option<oneshot::Sender<String>>>>>,
}

impl GrainHub {
  fn new() -> Self {
    Self { responder: None }
  }
}

impl Actor for GrainHub {
  fn receive(&mut self, ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(start) = message.downcast_ref::<StartGrainCall>() {
      let envelope = AnyMessage::new(GrainCall {
        key:      start.key.clone(),
        body:     start.body.clone(),
        reply_to: ctx.self_ref(),
      });
      start.target.tell(envelope).map_err(|e| ActorError::recoverable(format!("remote send failed: {e:?}")))?;
      return Ok(());
    }

    if let Some(register) = message.downcast_ref::<RegisterResponder>() {
      self.responder = Some(register.tx.clone());
      return Ok(());
    }

    if let Some(call) = message.downcast_ref::<GrainCall>() {
      println!("[hub] recv grain call key={} body={}", call.key, call.body);
      let props = Props::from_fn({
        let reply_to = call.reply_to.clone();
        let body = call.body.clone();
        move || GrainActor::new(reply_to.clone(), body.clone())
      })
      .with_name(format!("grain-{}", sanitize_key(&call.key)));
      ctx.spawn_child(&props).map_err(|e| ActorError::recoverable(format!("spawn failed: {e:?}")))?;
      return Ok(());
    }

    if let Some(reply) = message.downcast_ref::<GrainReply>() {
      if let Some(tx) = &self.responder {
        if let Ok(mut guard) = tx.lock() {
          if let Some(sender) = guard.take() {
            let _ = sender.send(reply.body.clone());
          }
        }
      }
      return Ok(());
    }

    Ok(())
  }
}

struct GrainActor {
  reply_to: ActorRef,
  body:     String,
}

impl GrainActor {
  fn new(reply_to: ActorRef, body: String) -> Self {
    Self { reply_to, body }
  }
}

impl Actor for GrainActor {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_, '_>) -> Result<(), ActorError> {
    println!("[grain] start");
    self
      .reply_to
      .tell(AnyMessage::new(GrainReply { body: format!("echo:{0}", self.body) }))
      .map_err(|e| ActorError::recoverable(format!("reply failed: {e:?}")))?;
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_, '_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[derive(Clone)]
struct StartGrainCall {
  target: ActorRef,
  key:    String,
  body:   String,
}

#[derive(Clone)]
struct GrainCall {
  key:      String,
  body:     String,
  reply_to: ActorRef,
}

#[derive(Clone)]
struct GrainReply {
  body: String,
}

#[derive(Clone)]
struct RegisterResponder {
  tx: Arc<Mutex<Option<oneshot::Sender<String>>>>,
}

fn sanitize_key(key: &str) -> String {
  key.replace(['/', ':'], "_")
}
