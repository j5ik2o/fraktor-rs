#![allow(clippy::print_stdout)]

//! Cluster extension quickstart (no_std + StaticClusterProvider)
//!
//! # 概要
//!
//! このサンプルは EventStream 主導のトポロジ通知方式を no_std 環境で実装しています：
//! 1. `StaticClusterProvider` が `ClusterEvent::TopologyUpdated` を EventStream に publish
//! 2. `ClusterExtension` が EventStream を購読し、自動的に `ClusterCore::on_topology` を呼び出す
//! 3. 手動の `on_topology` 呼び出しは不要
//!
//! # EventStream 方式
//!
//! ProtoActor-Go と同様の設計で、Provider/Gossiper がイベントを publish し、
//! ClusterExtension が購読して ClusterCore に適用する流れを採用しています。
//!
//! # Phase1 (静的トポロジ)
//!
//! `StaticClusterProvider` に静的トポロジを設定し、`start_member()` 時に publish します。
//! no_std 環境では GossipEngine を使用せず、静的トポロジのみで動作します。
//!
//! # Provider 差し替え方法
//!
//! `ClusterProvider` トレイトを実装することで、Provider を差し替えられます：
//! - `StaticClusterProvider`: no_std 環境向け静的トポロジ（本サンプル）
//! - `LocalClusterProvider`: std/Tokio 環境向け、Transport イベント自動検知
//! - etcd/zk/automanaged provider: 外部サービス連携（Phase2以降で対応予定）
//!
//! 詳細は `.kiro/specs/protoactor-go-cluster-extension-samples/example.md` を参照。
//!
//! # 実行例
//!
//! ```bash
//! cargo run -p fraktor-cluster-rs --example cluster_extension_no_std --features test-support
//! ```
//!
//! # 期待される出力
//!
//! ```text
//! === Cluster Extension No-Std Demo ===
//! Demonstrates EventStream-based topology with StaticClusterProvider
//! (No manual on_topology calls - topology is automatically published)
//!
//! --- Starting cluster members ---
//! [identity] setup_member: ["grain", "topic"]
//! [node-a] cluster started (mode=Member)
//! [node-a] topology updated: joined=["node-b"], left=[]
//! ...
//!
//! --- Checking metrics after startup ---
//! [node-a] members=2, virtual_actors=2
//! [node-b] members=2, virtual_actors=2
//!
//! --- Sending grain message ---
//! [grain] recv: hello from node-a
//!
//! --- Shutting down ---
//! ...
//! === Demo complete ===
//! ```

#[cfg(not(feature = "test-support"))]
compile_error!("cluster_extension_no_std example requires --features test-support");

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric},
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  scheduler::{ManualTestDriver, TickDriverConfig},
  system::{ActorSystemConfig, ActorSystemGeneric},
};
use fraktor_cluster_rs::core::{
  ActivatedKind, ClusterEvent, ClusterExtensionConfig, ClusterExtensionGeneric, ClusterExtensionId, ClusterPubSub,
  ClusterTopology, Gossiper, IdentityLookup, IdentitySetupError, StaticClusterProvider,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

// デモ用の Gossiper（Phase1 では未使用）
#[derive(Default)]
struct DemoGossiper;
impl Gossiper for DemoGossiper {
  fn start(&mut self) -> Result<(), &'static str> {
    Ok(())
  }

  fn stop(&mut self) -> Result<(), &'static str> {
    Ok(())
  }
}

// デモ用の PubSub
#[derive(Default)]
struct DemoPubSub;
impl ClusterPubSub for DemoPubSub {
  fn start(&mut self) -> Result<(), fraktor_cluster_rs::core::PubSubError> {
    Ok(())
  }

  fn stop(&mut self) -> Result<(), fraktor_cluster_rs::core::PubSubError> {
    Ok(())
  }
}

// デモ用の IdentityLookup
#[derive(Default)]
struct DemoIdentityLookup;
impl IdentityLookup for DemoIdentityLookup {
  fn setup_member(&mut self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    let names: Vec<_> = kinds.iter().map(|k| k.name().to_string()).collect();
    println!("[identity] setup_member: {:?}", names);
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }
}

// デモ用の BlockList
#[derive(Default)]
struct DemoBlockList;
impl BlockListProvider for DemoBlockList {
  fn blocked_members(&self) -> Vec<String> {
    vec!["blocked.demo".into()]
  }
}

// EventStream を購読してクラスタイベントをログに出力する subscriber
struct ClusterEventLogger {
  node_name: &'static str,
}

impl ClusterEventLogger {
  const fn new(node_name: &'static str) -> Self {
    Self { node_name }
  }
}

impl EventStreamSubscriber<NoStdToolbox> for ClusterEventLogger {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    if let EventStreamEvent::Extension { name, payload } = event {
      if name == "cluster" {
        if let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>() {
          match cluster_event {
            | ClusterEvent::Startup { mode, .. } => {
              println!("[{}] cluster started (mode={:?})", self.node_name, mode);
            },
            | ClusterEvent::TopologyUpdated { joined, left, .. } => {
              println!("[{}] topology updated: joined={:?}, left={:?}", self.node_name, joined, left);
            },
            | ClusterEvent::Shutdown { .. } => {
              println!("[{}] cluster shutdown", self.node_name);
            },
            | _ => {},
          }
        }
      }
    }
  }
}

// デモ用の Grain アクター
struct GrainActor;
impl Actor for GrainActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), fraktor_actor_rs::core::error::ActorError> {
    if let Some(text) = message.downcast_ref::<String>() {
      println!("[grain] recv: {text}");
    }
    Ok(())
  }
}

// ノードのコンポーネントをまとめる構造体
struct ClusterNode {
  system:    ActorSystemGeneric<NoStdToolbox>,
  extension: ArcShared<ClusterExtensionGeneric<NoStdToolbox>>,
  driver:    ManualTestDriver<NoStdToolbox>,
}

impl ClusterNode {
  fn new(name: &'static str, peer_name: &str) -> Self {
    // ActorSystem を構築
    let driver = ManualTestDriver::<NoStdToolbox>::new();
    let tick_cfg = TickDriverConfig::manual(driver.clone());
    let system_cfg =
      ActorSystemConfig::default().with_system_name(format!("cluster-{}", name)).with_tick_driver(tick_cfg);

    let grain_props = Props::from_fn(|| GrainActor).with_name("grain");
    let system: ActorSystemGeneric<NoStdToolbox> =
      ActorSystemGeneric::new_with_config(&grain_props, &system_cfg).expect("system build");

    // EventStream のサブスクライバを登録（クラスタイベントを観測）
    let event_subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> =
      ArcShared::new(ClusterEventLogger::new(name));
    let _subscription = system.subscribe_event_stream(&event_subscriber);

    // 静的トポロジを設定した StaticClusterProvider を作成
    // start_member() 時に EventStream へ TopologyUpdated を自動 publish する
    let static_topology = ClusterTopology::new(1, vec![peer_name.to_string()], vec![]);
    let provider = StaticClusterProvider::new(system.event_stream(), ArcShared::new(DemoBlockList::default()), name)
      .with_static_topology(static_topology);

    // ClusterExtension を登録
    let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
      ClusterExtensionConfig::new().with_advertised_address(name).with_metrics_enabled(true),
      Box::new(provider),
      ArcShared::new(DemoBlockList::default()),
      Box::new(DemoGossiper::default()),
      Box::new(DemoPubSub::default()),
      Box::new(DemoIdentityLookup::default()),
    );

    let extension = system.extended().register_extension(&ext_id);
    extension.setup_member_kinds(vec![ActivatedKind::new("grain")]).expect("kinds");

    Self { system, extension, driver }
  }

  fn start(&self) {
    // start_member() で InprocSampleProvider が TopologyUpdated を自動 publish
    // ClusterExtension が EventStream を購読しているので自動的に apply_topology が呼ばれる
    self.extension.start_member().expect("start member");
  }

  fn tick(&self, count: usize) {
    let controller = self.driver.controller();
    for _ in 0..count {
      controller.inject_and_drive(1);
    }
  }

  fn shutdown(&self) {
    self.extension.shutdown(true).ok();
  }

  fn metrics(&self) -> (usize, i64) {
    let m = self.extension.metrics().expect("metrics");
    (m.members(), m.virtual_actors())
  }

  fn send_message(&self, msg: String) {
    let grain_ref = self.system.user_guardian_ref();
    grain_ref.tell(AnyMessage::new(msg)).expect("tell");
    self.tick(3);
  }
}

fn main() {
  println!("=== Cluster Extension No-Std Demo ===");
  println!("Demonstrates EventStream-based topology with StaticClusterProvider");
  println!("(No manual on_topology calls - topology is automatically published)\n");

  // 1. 2ノードを作成（それぞれ相手をピアとして静的トポロジを設定）
  let node_a = ClusterNode::new("node-a", "node-b");
  let node_b = ClusterNode::new("node-b", "node-a");

  // 2. クラスタを開始
  //    - start_member() で StaticClusterProvider が TopologyUpdated を EventStream に publish
  //    - ClusterExtension が自動的に購読してトポロジを適用
  println!("--- Starting cluster members ---");
  node_a.start();
  node_b.start();

  // tick を回してイベント処理
  node_a.tick(3);
  node_b.tick(3);

  // 3. メトリクスを確認（トポロジが自動適用されたことを確認）
  println!("\n--- Checking metrics after startup ---");
  let (members_a, va_a) = node_a.metrics();
  let (members_b, va_b) = node_b.metrics();
  println!("[node-a] members={}, virtual_actors={}", members_a, va_a);
  println!("[node-b] members={}, virtual_actors={}", members_b, va_b);

  // 4. メッセージ送信
  println!("\n--- Sending grain message ---");
  node_a.send_message("hello from node-a".to_string());

  // 5. シャットダウン
  println!("\n--- Shutting down ---");
  node_b.shutdown();
  node_a.shutdown();

  // tick を回してシャットダウンイベント処理
  node_a.tick(3);
  node_b.tick(3);

  println!("\n=== Demo complete ===");
}
