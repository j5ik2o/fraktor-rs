use alloc::string::String;

use crate::{Entity, EntityContext, GrainTypeKey};

#[derive(Debug, Clone, PartialEq)]
struct UserMessage;

fn noop_create_behavior(_context: &EntityContext<UserMessage>) {}

fn build_kernel_context() -> fraktor_cluster_core_kernel_rs::grain::GrainContextImpl {
  use fraktor_cluster_core_kernel_rs::{
    activation::ClusterIdentity as KernelClusterIdentity, extension::ClusterApi, grain::GrainContextImpl,
  };

  let system = build_minimal_system();
  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = KernelClusterIdentity::new("user", "alice").expect("identity");
  GrainContextImpl::new(identity, api)
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

fn extract_user_id(message: &UserMessage) -> Option<String> {
  let _ = message;
  Some(String::from("user-7"))
}

#[test]
fn new_stores_type_key_and_behavior_factory() {
  let type_key = GrainTypeKey::<UserMessage>::new("user");
  let entity = Entity::new(type_key.clone(), noop_create_behavior);

  assert_eq!(entity.type_key(), &type_key);
  entity.create_behavior()(&EntityContext::from_kernel(build_kernel_context()));
}

#[test]
fn default_entity_id_extractor_returns_none() {
  let entity = Entity::new(GrainTypeKey::<UserMessage>::new("user"), noop_create_behavior);
  assert_eq!(entity.extract_entity_id()(&UserMessage), None);
}

#[test]
fn with_entity_id_extractor_replaces_rule() {
  let entity = Entity::new(GrainTypeKey::<UserMessage>::new("user"), noop_create_behavior)
    .with_entity_id_extractor(extract_user_id);
  assert_eq!(entity.extract_entity_id()(&UserMessage), Some(String::from("user-7")));
}

#[test]
fn into_type_key_returns_wrapped_key() {
  let type_key = GrainTypeKey::<UserMessage>::new("account");
  let entity = Entity::new(type_key.clone(), noop_create_behavior);
  assert_eq!(entity.into_type_key(), type_key);
}
