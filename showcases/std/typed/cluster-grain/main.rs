//! Typed cluster grain sharding showcase.
//!
//! Demonstrates `ClusterSharding::init` and typed `GrainRef<M>` resolution.

use std::string::String;

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::actor::{extension::ExtensionInstallers, setup::ActorSystemConfig};
use fraktor_actor_core_typed_rs::{TypedActorSystem, TypedProps, dsl::Behaviors};
use fraktor_cluster_core_kernel_rs::{
  activation::{
    ActivatedKind, IdentityLookup, IdentitySetupError, LookupError, PlacementDecision, PlacementLocality,
    PlacementResolution,
  },
  cluster_provider::NoopClusterProvider,
  extension::{ClusterExtension, ClusterExtensionConfig, ClusterExtensionInstaller},
  grain::GrainKey,
};
use fraktor_cluster_core_typed_rs::{ClusterSharding, Entity, EntityContext, GrainTypeKey};

#[derive(Debug)]
struct CounterMessage;

fn noop_create_behavior(_context: &EntityContext<CounterMessage>) {}

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
    let pid = format!("{}::{}", self.authority, key.value());
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid,
    })
  }
}

fn main() {
  let cluster_installer = ClusterExtensionInstaller::new(
    ClusterExtensionConfig::new().with_advertised_address("127.0.0.1:7355"),
    |_event_stream, _block_list, _address| Box::new(NoopClusterProvider::new()),
  )
  .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("127.0.0.1:7355")));

  let installers = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_extension_installers(installers);
  let props = TypedProps::<CounterMessage>::from_behavior_factory(Behaviors::ignore);
  let system = TypedActorSystem::create_from_props(&props, config).expect("typed system");

  let extension = system.as_untyped().extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  extension.start_member().expect("start member");

  let sharding = ClusterSharding::get(&system).expect("cluster sharding");
  let entity = Entity::new(GrainTypeKey::<CounterMessage>::new("counter"), noop_create_behavior);
  let region = sharding.init(entity).expect("init");
  let grain_ref = region.entity_ref_for("user-1").expect("entity ref");

  println!("typed cluster grain: kind={} entity={}", grain_ref.identity().kind(), grain_ref.identity().identity());

  system.terminate().expect("terminate");
}
