//! Cluster membership observation.
//!
//! Demonstrates joining a cluster with `LocalClusterProvider` and
//! observing topology changes via `EventStream` subscription.
//! Two nodes start with static topology and receive `ClusterEvent`
//! notifications as the cluster state evolves.
//!
//! Run with:
//! ```bash
//! cargo run -p fraktor-showcases-std --features advanced --example cluster_membership
//! ```

#![allow(clippy::print_stdout)]

use anyhow::{Result, anyhow};
use fraktor_actor_adaptor_rs::std::{
  dispatch::dispatcher::{DispatcherConfig, dispatch_executor::TokioExecutor},
  system::ActorSystem,
};
use fraktor_actor_rs::core::kernel::{
  actor::{
    Actor, ActorContext, error::ActorError, extension::ExtensionInstallers, messaging::AnyMessageView, props::Props,
    setup::ActorSystemConfig,
  },
  event::stream::{EventStreamEvent, EventStreamSubscriber, EventStreamSubscription, subscriber_handle},
  serialization::SerializationExtensionInstaller,
  system::remote::RemotingConfig,
  util::futures::ActorFutureListener,
};
use fraktor_cluster_rs::core::{
  ClusterEvent, ClusterExtension, ClusterExtensionConfig, ClusterExtensionInstaller, ClusterTopology,
};
use fraktor_remote_rs::core::{
  RemotingExtensionInstaller,
  actor_ref_provider::{loopback::default_loopback_setup, tokio::TokioActorRefProviderInstaller},
  remoting_extension::RemotingExtensionConfig,
};
use fraktor_showcases_std::support::tokio_tick_driver_config;
use fraktor_utils_rs::core::sync::ArcShared;

const HOST: &str = "127.0.0.1";
const NODE_A_PORT: u16 = 26050;
const NODE_B_PORT: u16 = 26051;

// --- EventStream でクラスタイベントを監視するサブスクライバ ---

struct ClusterEventPrinter {
  label: String,
}

impl EventStreamSubscriber for ClusterEventPrinter {
  fn on_event(&mut self, event: &EventStreamEvent) {
    match event {
      | EventStreamEvent::Extension { name, payload } if name == "cluster" => {
        if let Some(cluster_event) = payload.as_view().downcast_ref::<ClusterEvent>() {
          println!("[cluster][{}] {:?}", self.label, cluster_event);
        }
      },
      | _ => {},
    }
  }
}

// --- ノードのライフサイクルを管理する構造体 ---

struct ClusterNode {
  system:              ActorSystem,
  cluster:             ArcShared<ClusterExtension>,
  // EventStream サブスクリプションを保持（ドロップで解除されるため）
  #[allow(dead_code)]
  _event_subscription: EventStreamSubscription,
}

// --- Guardian アクター（メッセージ受信用の最小実装）---

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

// --- クラスタノードの構築 ---

fn build_cluster_node(system_name: &str, port: u16, static_topology: ClusterTopology) -> Result<ClusterNode> {
  let tokio_handle = tokio::runtime::Handle::current();
  let tokio_executor = TokioExecutor::new(tokio_handle);
  let default_dispatcher = DispatcherConfig::from_executor(Box::new(tokio_executor));

  let advertised = format!("{HOST}:{port}");
  let cluster_config = ClusterExtensionConfig::default()
    .with_advertised_address(&advertised)
    .with_metrics_enabled(true)
    .with_static_topology(static_topology);

  let remoting_config = RemotingExtensionConfig::default().with_transport_scheme("fraktor.tcp");
  let system_config = ActorSystemConfig::default()
    .with_system_name(system_name.to_string())
    .with_tick_driver(tokio_tick_driver_config())
    .with_default_dispatcher(default_dispatcher.into_core())
    .with_actor_ref_provider_installer(TokioActorRefProviderInstaller::default())
    .with_remoting_config(RemotingConfig::default().with_canonical_host(HOST).with_canonical_port(port))
    .with_extension_installers(
      ExtensionInstallers::default()
        .with_extension_installer(SerializationExtensionInstaller::new(default_loopback_setup()))
        .with_extension_installer(RemotingExtensionInstaller::new(remoting_config))
        .with_extension_installer(ClusterExtensionInstaller::new_with_local(cluster_config)),
    );

  let guardian = Props::from_fn(|| GuardianActor);
  let system =
    ActorSystem::new_with_config(&guardian, &system_config).map_err(|e| anyhow!("system build failed: {e:?}"))?;

  // EventStream を購読してクラスタイベントを表示
  let event_subscriber = subscriber_handle(ClusterEventPrinter { label: system_name.to_string() });
  let event_subscription = system.subscribe_event_stream(&event_subscriber);

  let cluster = system.extended().extension_by_type::<ClusterExtension>().expect("ClusterExtension not installed");

  Ok(ClusterNode { system, cluster, _event_subscription: event_subscription })
}

#[tokio::main]
async fn main() -> Result<()> {
  println!("=== Cluster Membership Demo ===");
  println!("2ノードの静的トポロジでクラスタを構成し、EventStreamでイベントを観測\n");

  // ノードA: ピアとしてノードBのアドレスを登録
  let topology_a = ClusterTopology::new(1, vec![format!("{HOST}:{NODE_B_PORT}")], Vec::new(), Vec::new());
  let node_a = build_cluster_node("node-a", NODE_A_PORT, topology_a)?;

  // ノードB: ピアとしてノードAのアドレスを登録
  let topology_b = ClusterTopology::new(2, vec![format!("{HOST}:{NODE_A_PORT}")], Vec::new(), Vec::new());
  let node_b = build_cluster_node("node-b", NODE_B_PORT, topology_b)?;

  // クラスタをメンバーモードで起動
  // LocalClusterProvider が静的トポロジを EventStream に publish し、
  // ClusterExtension が自動的に購読して ClusterCore に適用する
  println!("--- クラスタメンバーの起動 ---");
  node_a.cluster.start_member().map_err(|e| anyhow!("start_member node-a: {e:?}"))?;
  node_b.cluster.start_member().map_err(|e| anyhow!("start_member node-b: {e:?}"))?;

  // クラスタ収束を待機（EventStream 経由のトポロジ適用が完了するのを待つ）
  tokio::time::sleep(std::time::Duration::from_millis(200)).await;

  // メトリクスを確認
  println!("\n--- メトリクス確認 ---");
  let metrics_a = node_a.cluster.metrics().map_err(|e| anyhow!("metrics node-a: {e:?}"))?;
  let metrics_b = node_b.cluster.metrics().map_err(|e| anyhow!("metrics node-b: {e:?}"))?;
  println!("[node-a] members={}, virtual_actors={}", metrics_a.members(), metrics_a.virtual_actors());
  println!("[node-b] members={}, virtual_actors={}", metrics_b.members(), metrics_b.virtual_actors());

  // シャットダウン
  println!("\n--- シャットダウン ---");
  node_b.cluster.shutdown(true).map_err(|e| anyhow!("shutdown node-b: {e:?}"))?;
  node_a.cluster.shutdown(true).map_err(|e| anyhow!("shutdown node-a: {e:?}"))?;

  // ActorSystem の終了待機
  node_a.system.terminate().map_err(|e| anyhow!("terminate node-a: {e:?}"))?;
  node_b.system.terminate().map_err(|e| anyhow!("terminate node-b: {e:?}"))?;
  ActorFutureListener::new(node_a.system.when_terminated()).await;
  ActorFutureListener::new(node_b.system.when_terminated()).await;

  println!("\n=== Demo complete ===");
  Ok(())
}
