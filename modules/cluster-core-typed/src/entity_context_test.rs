use fraktor_cluster_core_kernel_rs::{
  activation::ClusterIdentity as KernelClusterIdentity,
  extension::ClusterApi,
  grain::{GrainContext, GrainContextImpl},
};

use crate::{EntityContext, GrainTypeKey};

#[derive(Debug, PartialEq)]
struct CounterMessage;

#[test]
fn from_kernel_exposes_kind_entity_id_and_cluster_api() {
  let api = ClusterApi::try_from_system(&build_minimal_system()).expect("cluster api");
  let identity = KernelClusterIdentity::new("counter", "entity-1").expect("identity");
  let context = EntityContext::<CounterMessage>::from_kernel(GrainContextImpl::new(identity, api.clone()));

  assert_eq!(context.kind(), "counter");
  assert_eq!(context.entity_id(), "entity-1");
  assert_eq!(context.type_key(), GrainTypeKey::<CounterMessage>::new("counter"));
  assert!(core::ptr::eq(context.cluster(), context.as_kernel().cluster()));
}

#[test]
fn into_kernel_roundtrip_preserves_identity() {
  let api = ClusterApi::try_from_system(&build_minimal_system()).expect("cluster api");
  let identity = KernelClusterIdentity::new("order", "order-42").expect("identity");
  let typed = EntityContext::<CounterMessage>::from_kernel(GrainContextImpl::new(identity.clone(), api));

  let kernel = typed.into_kernel();
  assert_eq!(kernel.kind(), identity.kind());
  assert_eq!(kernel.identity(), identity.identity());
}

fn build_minimal_system() -> fraktor_actor_core_kernel_rs::system::ActorSystem {
  use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
  use fraktor_actor_core_kernel_rs::{
    actor::{
      Actor, ActorContext, error::ActorError, extension::ExtensionInstallers, messaging::AnyMessageView, props::Props,
      setup::ActorSystemConfig,
    },
    system::ActorSystem,
  };
  use fraktor_cluster_core_kernel_rs::{
    cluster_provider::NoopClusterProvider,
    extension::{ClusterExtensionConfig, ClusterExtensionInstaller},
  };

  struct TestGuardian;
  impl Actor for TestGuardian {
    fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
      Ok(())
    }
  }

  let cluster_installer = ClusterExtensionInstaller::new(
    ClusterExtensionConfig::new().with_advertised_address("node1:8080"),
    |_event_stream, _block_list, _address| Box::new(NoopClusterProvider::new()),
  );
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_extension_installers(extensions);
  let props = Props::from_fn(|| TestGuardian);
  ActorSystem::create_from_props(&props, config).expect("build system")
}
