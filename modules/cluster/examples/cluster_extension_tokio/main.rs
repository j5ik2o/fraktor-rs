#![allow(clippy::print_stdout)]

//! Cluster extension quickstart (Tokio + LocalClusterProvider)
//!
//! # 概要
//!
//! このサンプルは EventStream 主導のトポロジ通知方式を実装しています：
//! 1. `LocalClusterProvider` が `ClusterEvent::TopologyUpdated` を EventStream に publish
//! 2. `ClusterExtension` が EventStream を購読し、自動的に `ClusterCore::on_topology` を呼び出す
//! 3. 手動の `on_topology` 呼び出しは不要
//!
//! # EventStream 方式
//!
//! ProtoActor-Go と同様の設計で、Provider/Gossiper がイベントを publish し、
//! ClusterExtension が購読して ClusterCore に適用する流れを採用しています。
//!
//! # Phase1 (静的トポロジ) vs Phase2 (動的トポロジ)
//!
//! - **Phase1**: `LocalClusterProvider` に静的トポロジを設定し、`start_member()` 時に publish
//! - **Phase2**: `subscribe_remoting_events()` で Transport イベント（Connected/Quarantined）を
//!   自動検知し、動的にトポロジを更新
//!
//! # Provider 差し替え方法
//!
//! `ClusterProvider` トレイトを実装することで、Provider を差し替えられます：
//! - `LocalClusterProvider`: 静的トポロジ + Transport イベント自動検知（本サンプル）
//! - `StaticClusterProvider`: no_std 環境向け静的トポロジ
//! - etcd/zk/automanaged provider: 外部サービス連携（Phase2以降で対応予定）
//!
//! 詳細は `.kiro/specs/protoactor-go-cluster-extension-samples/example.md` を参照。
//!
//! # 実行例
//!
//! ```bash
//! cargo run -p fraktor-cluster-rs --example cluster_extension_tokio --features std
//! ```
//!
//! # 期待される出力
//!
//! ```text
//! === Cluster Extension Tokio Demo ===
//! Demonstrates EventStream-based topology with LocalClusterProvider
//!
//! --- Starting cluster members ---
//! [identity][cluster-node-a] member kinds: ["grain", "topic"]
//! [pubsub][cluster-node-a] start
//! [gossip][cluster-node-a] start (no-op in Phase1)
//! [cluster][cluster-node-a] Startup { address: "127.0.0.1:26050", mode: Member }
//! [cluster][cluster-node-a] TopologyUpdated { ... }
//!
//! --- Checking metrics after startup ---
//! [node-a] members=2, virtual_actors=2
//! [node-b] members=2, virtual_actors=2
//!
//! --- Sending grain call ---
//! [hub] recv grain call key=user:va-1 body=hello cluster over tokio tcp
//! [grain] start
//! [ok] grain reply: echo:hello cluster over tokio tcp
//!
//! --- Shutting down ---
//! ...
//! === Demo complete ===
//! ```

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
    event_stream::{EventStreamEvent, EventStreamSubscriber, EventStreamSubscription},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick::TickDriverConfig,
    system::{ActorSystem, ActorSystemConfig},
  },
};
use fraktor_cluster_rs::core::{
  ActivatedKind, ClusterEvent, ClusterExtensionConfig, ClusterExtensionGeneric, ClusterExtensionInstaller,
  ClusterTopology,
};
use fraktor_remote_rs::core::{
  RemotingExtensionConfig, RemotingExtensionInstaller, TokioActorRefProviderInstaller, TokioTransportConfig,
  default_loopback_setup,
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
  println!("Demonstrates EventStream-based topology with LocalClusterProvider\n");

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
  system:              ActorSystem,
  cluster:             ArcShared<fraktor_cluster_rs::core::ClusterExtensionGeneric<StdToolbox>>,
  // EventStream サブスクリプションを保持（ドロップされると解除されるため）
  #[allow(dead_code)]
  _event_subscription: EventStreamSubscription,
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

  // ClusterExtensionInstaller を作成（static_topology を設定）
  // new_with_local() を使用して LocalClusterProvider を自動的に作成
  let advertised = format!("{HOST}:{port}");
  let mut cluster_config =
    ClusterExtensionConfig::default().with_advertised_address(&advertised).with_metrics_enabled(true);
  if let Some(topology) = static_topology {
    cluster_config = cluster_config.with_static_topology(topology);
  }

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
        .with_extension_installer(RemotingExtensionInstaller::new(remoting_config.clone()))
        .with_extension_installer(ClusterExtensionInstaller::new_with_local(cluster_config)),
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

  let event_subscriber: ArcShared<dyn EventStreamSubscriber> =
    ArcShared::new(ClusterEventPrinter::new(system_name.to_string()));
  let event_subscription = system.subscribe_event_stream(&event_subscriber);

  let cluster = system
    .extended()
    .extension_by_type::<ClusterExtensionGeneric<StdToolbox>>()
    .expect("ClusterExtension not installed. Call install() first.");

  Ok(ClusterNode { system, cluster, _event_subscription: event_subscription })
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
