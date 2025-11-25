#![allow(clippy::print_stdout)]

//! Cluster extension quickstart (Tokio + SampleTcpProvider)
//!
//! This example demonstrates using SampleTcpProvider to publish topology events
//! to EventStream. ClusterExtension automatically subscribes to these events
//! and applies topology updates to ClusterCore.
//!
//! Task 4.5: Transport のコネクション/切断イベントを SampleTcpProvider が自動検知し、
//! `TopologyUpdated` を publish する機能を実装。`subscribe_remoting_events()` を
//! 呼び出すことで、`RemotingLifecycleEvent::Connected` と `Quarantined` を自動的に
//! 検知し、トポロジ更新を行います。
//!
//! Provider差し替え方法:
//! - SampleTcpProvider: 静的トポロジ + Transport イベント自動検知（本サンプル）
//! - etcd/zk/automanaged provider: 外部サービス連携（Phase2以降で対応予定）
//!
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
  std::sample_tcp_provider::SampleTcpProvider,
};
use fraktor_remote_rs::core::{
  BlockListProvider, RemotingExtensionConfig, RemotingExtensionInstaller, TokioActorRefProviderInstaller,
  TokioTransportConfig, default_loopback_setup,
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
  println!("=== Cluster Extension Tokio Demo ===");
  println!("Demonstrates EventStream-based topology with SampleTcpProvider\n");

  // 返信待機チャネル（ノードB→ノードA→ノードB）
  let (reply_tx, reply_rx) = oneshot::channel::<String>();
  let shared_reply = Arc::new(Mutex::new(Some(reply_tx)));

  // ノードA: 受信・Grain 起動側（静的トポロジ: node-b が join）
  let static_topology_a = ClusterTopology::new(1, vec![format!("{HOST}:{NODE_B_PORT}")], Vec::new());
  let node_a = build_cluster_node("cluster-node-a", NODE_A_PORT, None, Some(static_topology_a))?;

  // ノードB: 送信・返信受信側（静的トポロジ: node-a が join）
  let static_topology_b = ClusterTopology::new(2, vec![format!("{HOST}:{NODE_A_PORT}")], Vec::new());
  let node_b = build_cluster_node("cluster-node-b", NODE_B_PORT, Some(shared_reply.clone()), Some(static_topology_b))?;

  // Kind を登録
  node_a
    .cluster
    .setup_member_kinds(vec![ActivatedKind::new(GRAIN_KIND)])
    .map_err(|e| anyhow!("identity setup (node a): {e:?}"))?;
  node_b
    .cluster
    .setup_member_kinds(vec![ActivatedKind::new(GRAIN_KIND)])
    .map_err(|e| anyhow!("identity setup (node b): {e:?}"))?;

  // クラスタをメンバーモードで起動
  // start_member() で SampleTcpProvider が静的トポロジを EventStream に publish
  // ClusterExtension が自動的に購読して apply_topology を呼ぶ
  println!("--- Starting cluster members ---");
  node_a.cluster.start_member().map_err(|e| anyhow!("start_member node a: {e:?}"))?;
  node_b.cluster.start_member().map_err(|e| anyhow!("start_member node b: {e:?}"))?;

  // メトリクスを確認（静的トポロジが自動適用されたことを確認）
  println!("\n--- Checking metrics after startup ---");
  let metrics_a = node_a.cluster.metrics().map_err(|e| anyhow!("metrics node a: {e:?}"))?;
  let metrics_b = node_b.cluster.metrics().map_err(|e| anyhow!("metrics node b: {e:?}"))?;
  println!("[node-a] members={}, virtual_actors={}", metrics_a.members(), metrics_a.virtual_actors());
  println!("[node-b] members={}, virtual_actors={}", metrics_b.members(), metrics_b.virtual_actors());

  println!("\n--- Transport-driven topology updates enabled ---");
  println!("(Connected/Quarantined events will automatically trigger TopologyUpdated)");

  // Grain 呼び出し（ノードBからノードAへリモート送信）
  println!("\n--- Sending grain call ---");
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
  println!("\n--- Shutting down ---");
  node_b.cluster.shutdown(true).map_err(|e| anyhow!("shutdown node b: {e:?}"))?;
  node_a.cluster.shutdown(true).map_err(|e| anyhow!("shutdown node a: {e:?}"))?;
  drop(node_b.system);
  drop(node_a.system);
  thread::sleep(Duration::from_millis(200));

  println!("\n=== Demo complete ===");
  Ok(())
}

struct ClusterNode {
  system:     ActorSystem,
  cluster:    ArcShared<fraktor_cluster_rs::core::ClusterExtensionGeneric<StdToolbox>>,
  // Task 4.5: provider は subscribe_remoting_events() 後に自動的に Transport イベントを監視
  // 手動の on_member_join/on_member_leave 呼び出しは不要になったが、
  // デバッグや拡張用途のために保持
  #[allow(dead_code)]
  provider:   ArcShared<SampleTcpProvider>,
  #[allow(dead_code)]
  advertised: String,
}

fn build_cluster_node(
  system_name: &str,
  port: u16,
  responder: Option<Arc<Mutex<Option<oneshot::Sender<String>>>>>,
  static_topology: Option<ClusterTopology>,
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

  if let Some(tx) = responder {
    system
      .user_guardian_ref()
      .tell(AnyMessage::new(RegisterResponder { tx }))
      .map_err(|e| anyhow!("register responder failed: {e:?}"))?;
  }

  // EventStream のサブスクライバを登録（クラスタイベントを観測）
  let event_subscriber: ArcShared<dyn EventStreamSubscriber> =
    ArcShared::new(ClusterEventPrinter::new(system_name.to_string()));
  let _subscription = system.subscribe_event_stream(&event_subscriber);

  // SampleTcpProvider を作成（EventStream publish 方式）
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockListProvider);
  let advertised = format!("{HOST}:{port}");
  let mut provider_builder = SampleTcpProvider::new(system.event_stream(), block_list.clone(), &advertised);
  if let Some(topology) = static_topology {
    provider_builder = provider_builder.with_static_topology(topology);
  }
  let provider = ArcShared::new(provider_builder);

  // Task 4.5: Transport イベントを自動検知するためのサブスクリプションを開始
  // RemotingLifecycleEvent::Connected/Quarantined を監視し、
  // 自動的に TopologyUpdated を publish する
  SampleTcpProvider::subscribe_remoting_events(&provider);

  let gossiper: ArcShared<dyn Gossiper> = ArcShared::new(LoggingGossiper::new(system_name));
  let pubsub: ArcShared<dyn ClusterPubSub> = ArcShared::new(LoggingPubSub::new(system_name));
  let identity: ArcShared<dyn IdentityLookup> = ArcShared::new(LoggingIdentityLookup::new(system_name));

  let cluster_config =
    ClusterExtensionConfig::default().with_advertised_address(advertised.clone()).with_metrics_enabled(true);
  let cluster_id =
    ClusterExtensionId::<StdToolbox>::new(cluster_config, provider.clone(), block_list, gossiper, pubsub, identity);
  let cluster = system.extended().register_extension(&cluster_id);

  Ok(ClusterNode { system, cluster, provider, advertised })
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

// === Cluster dependencies ===

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
      println!("[gossip][{}] start (no-op in Phase1)", self.label);
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
