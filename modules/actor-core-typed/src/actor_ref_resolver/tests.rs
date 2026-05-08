use alloc::string::String;

use fraktor_actor_core_rs::core::kernel::actor::{
  Actor, ActorCell, ActorContext, Pid, error::ActorError, extension::ExtensionInstallers, messaging::AnyMessageView,
  props::Props, setup::ActorSystemConfig,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

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
  let system = fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system();
  let pid = Pid::new(200, 0);
  let props = Props::from_fn(|| NoopActor);
  let cell = ActorCell::create(system.state(), pid, None, String::from("worker"), &props).expect("create worker cell");
  system.state().register_cell(cell.clone());
  let resolver = ActorRefResolver::new(&system);
  let serialized = resolver.to_serialization_format(&cell.actor_ref()).expect("serialize");
  let resolved = resolver.resolve_actor_ref(&serialized).expect("resolve");

  assert_eq!(resolved.pid(), pid);
  system.state().remove_cell(&pid);
}

#[test]
fn actor_ref_resolver_rejects_actor_refs_from_another_actor_system() {
  let resolver_system = fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system();
  let foreign_system = fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system();
  let pid = Pid::new(201, 0);
  let props = Props::from_fn(|| NoopActor);
  let cell =
    ActorCell::create(foreign_system.state(), pid, None, String::from("foreign"), &props).expect("create foreign cell");
  foreign_system.state().register_cell(cell.clone());

  let resolver = ActorRefResolver::new(&resolver_system);
  assert!(resolver.to_serialization_format(&cell.actor_ref()).is_err());
  foreign_system.state().remove_cell(&pid);
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
