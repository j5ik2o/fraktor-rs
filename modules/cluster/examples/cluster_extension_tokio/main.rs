#![allow(clippy::print_stdout)]

//! Cluster extension quickstart (Tokio + LocalClusterProviderGeneric)
//!
//! # 概要
//!
//! このサンプルは EventStream 主導のトポロジ通知方式を実装しています：
//! 1. `LocalClusterProviderGeneric` が `ClusterEvent::TopologyUpdated` を EventStream に publish
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
//! - **Phase1**: `LocalClusterProviderGeneric` に静的トポロジを設定し、`start_member()` 時に
//!   publish
//! - **Phase2**: `subscribe_remoting_events()` で Transport イベント（Connected/Quarantined）を
//!   自動検知し、動的にトポロジを更新
//!
//! # Provider 差し替え方法
//!
//! `ClusterProvider` トレイトを実装することで、Provider を差し替えられます：
//! - `LocalClusterProviderGeneric`: 静的トポロジ + Transport イベント自動検知（本サンプル）
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
//! Demonstrates EventStream-based topology with LocalClusterProviderGeneric
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

use std::{thread, time::Duration};

use anyhow::{Result, anyhow};
use fraktor_actor_rs::{
  core::{
    error::ActorError, extension::ExtensionInstallers, serialization::SerializationExtensionInstaller,
    system::remote::RemotingConfig,
  },
  std::{
    actor::{Actor, ActorContext},
    dispatch::dispatcher::{DispatcherConfig, dispatch_executor::TokioExecutor},
    event::stream::{EventStreamEvent, EventStreamSubscriber, EventStreamSubscription, subscriber_handle},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick::TickDriverConfig,
    system::{ActorSystem, ActorSystemConfig},
  },
};
use fraktor_cluster_rs::{
  core::{
    ClusterEvent, ClusterExtensionConfig, ClusterExtensionGeneric, ClusterExtensionInstaller, ClusterTopology,
    grain::GrainKey,
    identity::{ClusterIdentity, IdentityLookup, IdentitySetupError, LookupError},
    placement::{ActivatedKind, PlacementDecision, PlacementLocality, PlacementResolution},
  },
  std::{ClusterApi, GrainRef, default_grain_call_options},
};
use fraktor_remote_rs::core::{
  RemotingExtensionInstaller,
  actor_ref_provider::{loopback::default_loopback_setup, tokio::TokioActorRefProviderInstaller},
  remoting_extension::RemotingExtensionConfig,
  transport::TokioTransportConfig,
};
use fraktor_utils_rs::{
  core::sync::ArcShared,
  std::{StdSyncMutex, runtime_toolbox::StdToolbox},
};

const HOST: &str = "127.0.0.1";
const NODE_A_PORT: u16 = 26050;
const NODE_B_PORT: u16 = 26051;
const CLUSTER_SYSTEM_NAME: &str = "cluster-demo";
const HUB_NAME: &str = "grain-hub";
const GRAIN_KIND: &str = "grain";
const SAMPLE_KEY: &str = "user:va-1";

#[tokio::main]
async fn main() -> Result<()> {
  println!("=== Cluster Extension Tokio Demo ===");
  println!("Demonstrates EventStream-based topology with LocalClusterProviderGeneric\n");

  // ノードA: 受信・Grain 起動側（静的トポロジ: node-b が join）
  let static_topology_a = ClusterTopology::new(1, vec![format!("{HOST}:{NODE_B_PORT}")], Vec::new(), Vec::new());
  let node_a_authority = format!("{HOST}:{NODE_A_PORT}");
  let node_a = build_cluster_node(
    "cluster-node-a",
    NODE_A_PORT,
    Some(static_topology_a),
    node_a_authority.clone(),
    HubRole::Receiver,
  )?;

  // ノードB: 送信・返信受信側（静的トポロジ: node-a が join）
  let static_topology_b = ClusterTopology::new(2, vec![format!("{HOST}:{NODE_A_PORT}")], Vec::new(), Vec::new());
  let node_b =
    build_cluster_node("cluster-node-b", NODE_B_PORT, Some(static_topology_b), node_a_authority, HubRole::Sender)?;

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
    .tell(AnyMessage::new(StartGrainCall))
    .map_err(|e| anyhow!("start grain call failed: {e:?}"))?;
  tokio::time::sleep(Duration::from_millis(200)).await;

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
  static_topology: Option<ClusterTopology>,
  lookup_authority: String,
  role: HubRole,
) -> Result<ClusterNode> {
  let tokio_handle = tokio::runtime::Handle::current();
  let tokio_executor = TokioExecutor::new(tokio_handle);
  let default_dispatcher = DispatcherConfig::from_executor(ArcShared::new(StdSyncMutex::new(Box::new(tokio_executor))));

  // ClusterExtensionInstaller を作成（static_topology を設定）
  // new_with_local() を使用して LocalClusterProviderGeneric を自動的に作成
  let advertised = format!("{HOST}:{port}");
  let mut cluster_config =
    ClusterExtensionConfig::default().with_advertised_address(&advertised).with_metrics_enabled(true);
  if let Some(topology) = static_topology {
    cluster_config = cluster_config.with_static_topology(topology);
  }

  let hub_path = format!("user/{HUB_NAME}");
  let identity_lookup_factory = {
    let hub_path = hub_path.clone();
    let lookup_authority = lookup_authority.clone();
    move || {
      Box::new(StaticHubIdentityLookup::new(lookup_authority.clone(), hub_path.clone())) as Box<dyn IdentityLookup>
    }
  };
  let remoting_config = RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp");
  let system_config = ActorSystemConfig::default()
    .with_system_name(CLUSTER_SYSTEM_NAME.to_string())
    .with_tick_driver_config(TickDriverConfig::tokio_quickstart())
    .with_default_dispatcher_config(default_dispatcher)
    .with_actor_ref_provider_installer(TokioActorRefProviderInstaller::from_config(TokioTransportConfig::default()))
    .with_remoting_config(RemotingConfig::default().with_canonical_host(HOST).with_canonical_port(port))
    .with_extension_installers(
      ExtensionInstallers::default()
        .with_extension_installer(SerializationExtensionInstaller::new(default_loopback_setup()))
        .with_extension_installer(RemotingExtensionInstaller::new(remoting_config.clone()))
        .with_extension_installer(
          ClusterExtensionInstaller::new_with_local(cluster_config)
            .with_identity_lookup_factory(identity_lookup_factory),
        ),
    );

  let guardian = Props::from_fn(move || GrainHub::new(role)).with_name(HUB_NAME);
  let system = ActorSystem::new_with_config(&guardian, &system_config)
    .map_err(|e| anyhow!("actor system build failed ({system_name}): {e:?}"))?;

  let event_subscriber = subscriber_handle(ClusterEventPrinter::new(system_name.to_string()));
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
  fn on_event(&mut self, event: &EventStreamEvent) {
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

#[derive(Clone, Copy)]
enum HubRole {
  Sender,
  Receiver,
}

struct StartGrainCall;

struct GrainHub {
  role: HubRole,
}

impl GrainHub {
  fn new(role: HubRole) -> Self {
    Self { role }
  }
}

impl Actor for GrainHub {
  fn receive(&mut self, ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<StartGrainCall>().is_some() {
      if matches!(self.role, HubRole::Sender) {
        let api = ClusterApi::try_from_system(&ctx.system())
          .map_err(|e| ActorError::recoverable(format!("cluster api failed: {e:?}")))?;
        let identity = ClusterIdentity::new(GRAIN_KIND, SAMPLE_KEY)
          .map_err(|e| ActorError::recoverable(format!("identity error: {e:?}")))?;
        let grain_ref = GrainRef::new(api, identity).with_options(default_grain_call_options());
        let request = AnyMessage::new("hello cluster over tokio tcp".to_string());
        let sender = ctx.self_ref();
        if let Err(error) = grain_ref.request_with_sender(&request, &sender) {
          return Err(ActorError::recoverable(format!("grain request failed: {error:?}")));
        }
      }
      return Ok(());
    }

    if let Some(payload) = message.downcast_ref::<String>() {
      match self.role {
        | HubRole::Receiver => {
          println!("[hub] recv grain request body={payload}");
          let reply = format!("echo:{payload}");
          if ctx.sender().is_some() {
            ctx.reply(AnyMessage::new(reply)).map_err(|e| ActorError::recoverable(format!("reply failed: {e:?}")))?;
          } else {
            println!("[hub] sender missing; skip reply");
          }
        },
        | HubRole::Sender => {
          println!("[sender] recv grain reply: {payload}");
        },
      }
    }

    Ok(())
  }
}

struct StaticHubIdentityLookup {
  authority: String,
  hub_path:  String,
}

impl StaticHubIdentityLookup {
  fn new(authority: String, hub_path: String) -> Self {
    Self { authority, hub_path }
  }
}

impl IdentityLookup for StaticHubIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    let decision =
      PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now };
    let pid = format!("{}::{}", self.authority, self.hub_path);
    Ok(PlacementResolution { decision, locality: PlacementLocality::Local, pid })
  }
}
