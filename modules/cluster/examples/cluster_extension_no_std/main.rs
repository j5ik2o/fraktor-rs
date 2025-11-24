#![allow(clippy::print_stdout)]

#[cfg(not(feature = "test-support"))]
compile_error!("cluster_extension_no_std example requires --features test-support");

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric},
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  scheduler::{ManualTestDriver, TickDriverConfig},
  system::{ActorSystemConfig, ActorSystemGeneric},
};
use fraktor_cluster_rs::core::{
  ActivatedKind, ClusterExtensionConfig, ClusterExtensionId, ClusterProvider, ClusterProviderError, ClusterPubSub,
  ClusterTopology, Gossiper, IdentityLookup, IdentitySetupError,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

#[derive(Default)]
struct DemoProvider;
impl ClusterProvider for DemoProvider {
  fn start_member(&self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

#[derive(Default)]
struct DemoGossiper;
impl Gossiper for DemoGossiper {
  fn start(&self) -> Result<(), &'static str> {
    Ok(())
  }

  fn stop(&self) -> Result<(), &'static str> {
    Ok(())
  }
}

#[derive(Default)]
struct DemoPubSub;
impl ClusterPubSub for DemoPubSub {
  fn start(&self) -> Result<(), fraktor_cluster_rs::core::PubSubError> {
    Ok(())
  }

  fn stop(&self) -> Result<(), fraktor_cluster_rs::core::PubSubError> {
    Ok(())
  }
}

#[derive(Default)]
struct DemoIdentityLookup;
impl IdentityLookup for DemoIdentityLookup {
  fn setup_member(&self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }
}

#[derive(Default)]
struct DemoBlockList;
impl BlockListProvider for DemoBlockList {
  fn blocked_members(&self) -> Vec<String> {
    vec!["blocked.demo".into()]
  }
}

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
  let driver = ManualTestDriver::<NoStdToolbox>::new();
  let tick_cfg = TickDriverConfig::manual(driver.clone());
  let system_cfg = ActorSystemConfig::default().with_system_name("cluster-demo".to_string()).with_tick_driver(tick_cfg);

  let grain_props = Props::from_fn(|| GrainActor).with_name("grain");
  let system: ActorSystemGeneric<NoStdToolbox> =
    ActorSystemGeneric::new_with_config(&grain_props, &system_cfg).expect("system build");

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo").with_metrics_enabled(true),
    ArcShared::new(DemoProvider::default()),
    ArcShared::new(DemoBlockList::default()),
    ArcShared::new(DemoGossiper::default()),
    ArcShared::new(DemoPubSub::default()),
    ArcShared::new(DemoIdentityLookup::default()),
  );

  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("demo-kind")]).expect("kinds");
  ext_shared.start_member().expect("start member");

  ext_shared.on_topology(&ClusterTopology::new(1, vec!["node-b".into()], vec![]));

  // メッセージ送信 (user guardian 配下の grain アクター)
  let grain_ref = system.user_guardian_ref();
  grain_ref.tell(AnyMessage::new(String::from("hello cluster"))).expect("tell");

  // 手動 tick を少し回す
  let controller = driver.controller();
  for _ in 0..5 {
    controller.inject_and_drive(1);
  }

  ext_shared.shutdown(true).ok();
}
