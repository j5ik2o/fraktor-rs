use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::actor::{extension::ExtensionInstallers, setup::ActorSystemConfig};
use fraktor_actor_core_typed_rs::{TypedActorSystem, TypedProps, dsl::Behaviors};
use fraktor_cluster_core_kernel_rs::{
  cluster_provider::NoopClusterProvider,
  extension::{ClusterExtensionConfig, ClusterExtensionInstaller},
};

use crate::DistributedData;

fn build_typed_system() -> TypedActorSystem<()> {
  let cluster_installer = ClusterExtensionInstaller::new(
    ClusterExtensionConfig::new().with_advertised_address("node1:8080"),
    |_event_stream, _block_list, _address| Box::new(NoopClusterProvider::new()),
  );
  let installers = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_extension_installers(installers);
  let props = TypedProps::<()>::from_behavior_factory(Behaviors::ignore);
  TypedActorSystem::create_from_props(&props, config).expect("typed system")
}

#[test]
fn get_exposes_self_unique_address_from_cluster_authority() {
  let system = build_typed_system();
  let distributed_data = DistributedData::get(&system).expect("distributed data extension");
  assert_eq!(distributed_data.self_unique_address().unique_address().address().host(), "node1");
  assert_eq!(distributed_data.self_unique_address().unique_address().address().port(), 8080);
}

#[test]
fn default_unexpected_ask_timeout_is_sixty_seconds() {
  let system = build_typed_system();
  let distributed_data = DistributedData::get(&system).expect("distributed data extension");
  assert_eq!(distributed_data.unexpected_ask_timeout(), crate::DEFAULT_UNEXPECTED_ASK_TIMEOUT);
}
