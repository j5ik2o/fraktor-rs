use fraktor_actor_core_kernel_rs::actor::{
  Pid,
  actor_ref::{ActorRef, NullSender},
  extension::ExtensionInstallers,
  setup::ActorSystemConfig,
};

use crate::{
  TypedActorRef, TypedActorSystem, TypedProps,
  dsl::Behaviors,
  receptionist::{Receptionist, ReceptionistCommand, ReceptionistSetup},
};

#[test]
fn receptionist_setup_registers_custom_factory_during_bootstrap() {
  let custom_pid = Pid::new(9_001, 0);
  let installers = ExtensionInstallers::default().with_extension_installer(ReceptionistSetup::new(move |system| {
    let actor_ref = ActorRef::with_system(custom_pid, NullSender, &system.state());
    Receptionist::from_actor_ref(TypedActorRef::<ReceptionistCommand>::from_untyped(actor_ref))
  }));
  let config = ActorSystemConfig::new(crate::test_support::test_tick_driver()).with_extension_installers(installers);
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);

  let system = TypedActorSystem::<u32>::create_from_props(&guardian_props, config).expect("system");
  let receptionist = Receptionist::get(&system);

  assert_eq!(receptionist.r#ref().pid(), custom_pid);
  system.terminate().expect("terminate");
}
