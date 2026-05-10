use fraktor_actor_core_kernel_rs::actor::{
  Actor, ActorContext, error::ActorError, extension::ExtensionInstallers, messaging::AnyMessageView, props::Props,
  setup::ActorSystemConfig,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::ActorRefResolver;
use crate::{ActorRefResolverSetup, TypedActorSystem, TypedProps, actor_ref_resolver::ActorSystem, dsl::Behaviors};

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn actor_ref_resolver_serializes_and_resolves_spawned_actor_refs() {
  let system = fraktor_actor_adaptor_std_rs::system::new_noop_actor_system();
  let props = Props::from_fn(|| NoopActor);
  let child = system.actor_of_named(&props, "worker").expect("spawn worker");
  let resolver = ActorRefResolver::new(&system);
  let serialized = resolver.to_serialization_format(child.actor_ref()).expect("serialize");
  let resolved = resolver.resolve_actor_ref(&serialized).expect("resolve");

  assert_eq!(resolved.pid(), child.pid());
  system.state().remove_cell(&child.pid());
}

#[test]
fn actor_ref_resolver_rejects_actor_refs_from_another_actor_system() {
  let resolver_system = fraktor_actor_adaptor_std_rs::system::new_noop_actor_system();
  let foreign_system = fraktor_actor_adaptor_std_rs::system::new_noop_actor_system();
  let props = Props::from_fn(|| NoopActor);
  let child = foreign_system.actor_of_named(&props, "foreign").expect("spawn foreign");

  let resolver = ActorRefResolver::new(&resolver_system);
  assert!(resolver.to_serialization_format(child.actor_ref()).is_err());
  foreign_system.state().remove_cell(&child.pid());
}

#[test]
fn actor_ref_resolver_setup_overrides_default_extension_factory() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let invoked = ArcShared::new(SpinSyncMutex::new(false));
  let setup = ActorRefResolverSetup::new({
    let invoked = invoked.clone();
    move |system: &ActorSystem| {
      *invoked.lock() = true;
      ActorRefResolver::new(system)
    }
  });
  let config = ActorSystemConfig::new(crate::test_support::test_tick_driver())
    .with_extension_installers(ExtensionInstallers::default().with_extension_installer(setup));

  let system = TypedActorSystem::<u32>::create_from_props(&guardian_props, config).expect("system");
  assert!(ActorRefResolver::get(&system).is_some());
  assert!(*invoked.lock(), "custom resolver factory should be invoked");
  system.terminate().expect("terminate");
}
