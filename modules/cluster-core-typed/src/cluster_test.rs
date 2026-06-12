use alloc::{
  string::{String, ToString},
  vec,
};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::actor::{extension::ExtensionInstallers, setup::ActorSystemConfig};
use fraktor_actor_core_typed_rs::{TypedActorSystem, TypedProps, dsl::Behaviors};
use fraktor_cluster_core_kernel_rs::{
  activation::{
    ActivatedKind, IdentityLookup, IdentitySetupError, LookupError, PlacementDecision, PlacementLocality,
    PlacementResolution,
  },
  cluster_provider::NoopClusterProvider,
  extension::{ClusterApiError, ClusterExtension, ClusterExtensionConfig, ClusterExtensionInstaller},
  grain::GrainKey,
};

use crate::{Cluster, GrainTypeKey};

// ─── fixture ─────────────────────────────────────────────────────────────────

struct StaticIdentityLookup {
  authority: String,
}

impl StaticIdentityLookup {
  fn new(authority: &str) -> Self {
    Self { authority: authority.to_string() }
  }
}

impl IdentityLookup for StaticIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    let pid = alloc::format!("{}::{}", self.authority, key.value());
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid,
    })
  }
}

/// メッセージ型パラメータ用のテスト型。
#[derive(Debug)]
struct TestMsg;

// ─── テスト: typed bootstrap → Cluster::get → grain_ref_for ─────────────────

#[test]
fn grain_ref_for_builds_typed_grain_ref_with_matching_identity() {
  // typed bootstrap（cluster 拡張導入済み TypedActorSystem）を構築する。
  let cluster_installer =
    ClusterExtensionInstaller::new(ClusterExtensionConfig::new().with_advertised_address("node1:8080"), {
      |_event_stream, _block_list, _address| Box::new(NoopClusterProvider::new())
    })
    .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let installers = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let props = TypedProps::<TestMsg>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_extension_installers(installers);
  let system = TypedActorSystem::create_from_props(&props, config).expect("typed system");

  let extension = system.as_untyped().extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  extension.start_member().expect("start member");
  extension.setup_member_kinds(vec![ActivatedKind::new("counter")]).expect("setup kinds");

  // Cluster::get は拡張導入済みなので Ok になる。
  let cluster = Cluster::get(&system).expect("cluster");

  // GrainTypeKey → ClusterIdentity<M> を導出する。
  let type_key = GrainTypeKey::<TestMsg>::new("counter");
  let identity = type_key.identity_for("entity-1").expect("valid identity");

  // grain_ref_for で typed GrainRef<M> を構築する（要件 3.1）。
  let grain_ref = cluster.grain_ref_for(&identity);

  // postcondition: GrainRef::identity() が与えた識別と一致する。
  let returned_identity = grain_ref.identity();
  assert_eq!(returned_identity.kind(), identity.kind());
  assert_eq!(returned_identity.identity(), identity.identity());

  // postcondition: as_kernel().identity() が identity.as_kernel() と一致する。
  assert_eq!(grain_ref.as_kernel().identity(), identity.as_kernel());

  system.terminate().expect("terminate");
}

// ─── テスト: 拡張未導入の TypedActorSystem で Cluster::get が ExtensionNotInstalled ─

#[test]
fn cluster_get_returns_extension_not_installed_when_cluster_not_configured() {
  // cluster 拡張を導入しない TypedActorSystem を構築する。
  let props = TypedProps::<TestMsg>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::new(TestTickDriver::default());
  let system = TypedActorSystem::create_from_props(&props, config).expect("typed system");

  // cluster 拡張未導入 → get は Err(ExtensionNotInstalled) を返す（要件 3.2）。
  let result = Cluster::get(&system);
  assert!(
    matches!(result, Err(ClusterApiError::ExtensionNotInstalled)),
    "expected ExtensionNotInstalled, got: {:?}",
    result.err()
  );

  system.terminate().expect("terminate");
}

// ─── テスト: grain_ref_for postcondition — as_kernel().identity() が一致する ─

#[test]
fn grain_ref_for_postcondition_kernel_identity_matches() {
  let cluster_installer =
    ClusterExtensionInstaller::new(ClusterExtensionConfig::new().with_advertised_address("node1:8080"), {
      |_event_stream, _block_list, _address| Box::new(NoopClusterProvider::new())
    })
    .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let installers = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let props = TypedProps::<TestMsg>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_extension_installers(installers);
  let system = TypedActorSystem::create_from_props(&props, config).expect("typed system");

  let extension = system.as_untyped().extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  extension.start_member().expect("start member");
  extension.setup_member_kinds(vec![ActivatedKind::new("order")]).expect("setup kinds");

  let cluster = Cluster::get(&system).expect("cluster");
  let type_key = GrainTypeKey::<TestMsg>::new("order");
  let identity = type_key.identity_for("order-42").expect("valid identity");

  // grain_ref_for で typed GrainRef<M> を構築する。
  let grain_ref = cluster.grain_ref_for(&identity);

  // postcondition: as_kernel().identity() が identity.as_kernel() と一致する。
  assert_eq!(grain_ref.as_kernel().identity(), identity.as_kernel());

  system.terminate().expect("terminate");
}
