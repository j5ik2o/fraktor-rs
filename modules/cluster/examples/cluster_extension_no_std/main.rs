#![allow(clippy::print_stdout)]

//! Cluster extension quickstart (no_std + InprocSampleProvider)
//!
//! This example demonstrates using InprocSampleProvider to publish static topology
//! to EventStream, which ClusterExtension automatically subscribes to and applies
//! to ClusterCore without manual `on_topology` calls.
//!
//! Run with: `cargo run -p fraktor-cluster-rs --example cluster_extension_no_std --features
//! test-support`

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
  ActivatedKind, ClusterEvent, ClusterExtensionConfig, ClusterExtensionId, ClusterPubSub, ClusterTopology, Gossiper,
  IdentityLookup, IdentitySetupError, InprocSampleProvider,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

// デモ用の Gossiper（Phase1 では未使用）
#[derive(Default)]
struct DemoGossiper;
impl Gossiper for DemoGossiper {
  fn start(&self) -> Result<(), &'static str> {
    println!("[gossiper] start (no-op in Phase1)");
    Ok(())
  }

  fn stop(&self) -> Result<(), &'static str> {
    println!("[gossiper] stop");
    Ok(())
  }
}

// デモ用の PubSub
#[derive(Default)]
struct DemoPubSub;
impl ClusterPubSub for DemoPubSub {
  fn start(&self) -> Result<(), fraktor_cluster_rs::core::PubSubError> {
    println!("[pubsub] start");
    Ok(())
  }

  fn stop(&self) -> Result<(), fraktor_cluster_rs::core::PubSubError> {
    println!("[pubsub] stop");
    Ok(())
  }
}

// デモ用の IdentityLookup
#[derive(Default)]
struct DemoIdentityLookup;
impl IdentityLookup for DemoIdentityLookup {
  fn setup_member(&self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    let names: Vec<_> = kinds.iter().map(|k| k.name().to_string()).collect();
    println!("[identity] setup_member: {:?}", names);
    Ok(())
  }

  fn setup_client(&self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    let names: Vec<_> = kinds.iter().map(|k| k.name().to_string()).collect();
    println!("[identity] setup_client: {:?}", names);
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
struct ClusterEventLogger;
impl EventStreamSubscriber<NoStdToolbox> for ClusterEventLogger {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    if let EventStreamEvent::Extension { name, payload } = event {
      if name == "cluster" {
        if let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>() {
          println!("[cluster-event] {:?}", cluster_event);
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

fn main() {
  println!("=== Cluster Extension No-Std Demo ===");
  println!("Demonstrates EventStream-based topology with InprocSampleProvider\n");

  // 1. ActorSystem を構築
  let driver = ManualTestDriver::<NoStdToolbox>::new();
  let tick_cfg = TickDriverConfig::manual(driver.clone());
  let system_cfg = ActorSystemConfig::default().with_system_name("cluster-demo".to_string()).with_tick_driver(tick_cfg);

  let grain_props = Props::from_fn(|| GrainActor).with_name("grain");
  let system: ActorSystemGeneric<NoStdToolbox> =
    ActorSystemGeneric::new_with_config(&grain_props, &system_cfg).expect("system build");

  // 2. EventStream のサブスクライバを登録（クラスタイベントを観測）
  let event_subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = ArcShared::new(ClusterEventLogger);
  let _subscription = system.subscribe_event_stream(&event_subscriber);

  // 3. 静的トポロジを設定した InprocSampleProvider を作成 start_member() 時に EventStream へ
  //    TopologyUpdated を publish する
  let static_topology = ClusterTopology::new(1, vec!["node-b".into(), "node-c".into()], vec![]);
  let provider = InprocSampleProvider::new(system.event_stream(), ArcShared::new(DemoBlockList::default()), "node-a")
    .with_static_topology(static_topology);

  // 4. ClusterExtension を登録
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    ArcShared::new(provider),
    ArcShared::new(DemoBlockList::default()),
    ArcShared::new(DemoGossiper::default()),
    ArcShared::new(DemoPubSub::default()),
    ArcShared::new(DemoIdentityLookup::default()),
  );

  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("demo-kind")]).expect("kinds");

  // 5. クラスタを開始
  //    - start_member() で InprocSampleProvider が TopologyUpdated を publish
  //    - ClusterExtension が EventStream を購読しているので自動的に apply_topology が呼ばれる
  println!("\n--- Starting cluster member ---");
  ext_shared.start_member().expect("start member");

  // 6. メトリクスを確認（トポロジが自動適用されたことを確認）
  let metrics = ext_shared.metrics().expect("metrics");
  println!("\n[metrics] members={}, virtual_actors={}", metrics.members(), metrics.virtual_actors());
  println!("[metrics] blocked_members: {:?}", ext_shared.blocked_members());

  // 7. メッセージ送信
  let grain_ref = system.user_guardian_ref();
  grain_ref.tell(AnyMessage::new(String::from("hello cluster"))).expect("tell");

  // 8. 手動 tick を回す
  let controller = driver.controller();
  for _ in 0..5 {
    controller.inject_and_drive(1);
  }

  // 9. シャットダウン
  println!("\n--- Shutting down ---");
  ext_shared.shutdown(true).ok();

  println!("\n=== Demo complete ===");
}
