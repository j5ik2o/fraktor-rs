use alloc::string::String;

use fraktor_actor_core_kernel_rs::actor::{
  extension::{Extension, ExtensionInstallers},
  setup::ActorSystemConfig,
};
use fraktor_utils_core_rs::core::sync::shared::Shared;

use crate::{ExtensionSetup, TypedActorSystem, TypedProps, dsl::Behaviors, extension_setup::ActorSystem};

#[derive(Clone)]
struct ProbeExtensionId;

#[derive(Clone)]
struct ProbeExtension {
  value: String,
}

impl ProbeExtension {
  fn new(value: impl Into<String>) -> Self {
    Self { value: value.into() }
  }
}

impl Extension for ProbeExtension {}

impl fraktor_actor_core_kernel_rs::actor::extension::ExtensionId for ProbeExtensionId {
  type Ext = ProbeExtension;

  fn create_extension(&self, _system: &ActorSystem) -> Self::Ext {
    ProbeExtension::new("default")
  }
}

#[test]
fn extension_setup_registers_custom_factory_during_bootstrap() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let installers = ExtensionInstallers::default()
    .with_extension_installer(ExtensionSetup::new(ProbeExtensionId, |_system| ProbeExtension::new("custom")));
  let config = ActorSystemConfig::new(crate::test_support::test_tick_driver()).with_extension_installers(installers);

  let system = TypedActorSystem::<u32>::create_from_props(&guardian_props, config).expect("system");
  let extension = system
    .as_untyped()
    .extended()
    .extension(&ProbeExtensionId)
    .expect("extension should be installed during bootstrap");

  assert_eq!(extension.with_ref(|extension| extension.value.clone()), "custom");
  system.terminate().expect("terminate");
}
