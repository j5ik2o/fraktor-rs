use alloc::string::String;

use fraktor_utils_rs::core::sync::shared::Shared;

use crate::core::{
  extension::{Extension, ExtensionInstallers},
  scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
  system::ActorSystemConfig,
  typed::{Behaviors, ExtensionSetup, TypedActorSystem, TypedProps},
};

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

impl crate::core::extension::ExtensionId for ProbeExtensionId {
  type Ext = ProbeExtension;

  fn create_extension(&self, _system: &crate::core::system::ActorSystem) -> Self::Ext {
    ProbeExtension::new("default")
  }
}

#[test]
fn extension_setup_registers_custom_factory_during_bootstrap() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let installers = ExtensionInstallers::default()
    .with_extension_installer(ExtensionSetup::new(ProbeExtensionId, |_system| ProbeExtension::new("custom")));
  let config = ActorSystemConfig::default()
    .with_extension_installers(installers)
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()));

  let system = TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system");
  let extension = system
    .as_untyped()
    .extended()
    .extension(&ProbeExtensionId)
    .expect("extension should be installed during bootstrap");

  assert_eq!(extension.with_ref(|extension| extension.value.clone()), "custom");
  system.terminate().expect("terminate");
}
