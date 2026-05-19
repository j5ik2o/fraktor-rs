use alloc::{
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::hint::spin_loop;

use fraktor_actor_core_kernel_rs::actor::{
  error::ActorError,
  extension::{Extension, ExtensionInstallers},
  messaging::AnyMessage,
  setup::ActorSystemConfig,
  spawn::SpawnError,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex, shared::Shared};

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

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
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

#[test]
fn guardian_behavior_starts_after_extension_installers() {
  let observed = ArcShared::new(SpinSyncMutex::new(String::new()));
  let observed_factory = observed.clone();
  let guardian_props = TypedProps::<u32>::from_behavior_factory(move || {
    let observed_setup = observed_factory.clone();
    Behaviors::setup_result(move |ctx| {
      let Some(extension) = ctx.system().as_untyped().extended().extension(&ProbeExtensionId) else {
        return Err(ActorError::fatal("probe extension missing during guardian startup"));
      };
      *observed_setup.lock() = extension.with_ref(|extension| extension.value.clone());
      Ok(Behaviors::ignore())
    })
  });
  let installers = ExtensionInstallers::default()
    .with_extension_installer(ExtensionSetup::new(ProbeExtensionId, |_system| ProbeExtension::new("custom")));
  let config = ActorSystemConfig::new(crate::test_support::test_tick_driver()).with_extension_installers(installers);

  let system = TypedActorSystem::<u32>::create_from_props(&guardian_props, config).expect("system");

  wait_until(|| observed.lock().as_str() == "custom");
  system.terminate().expect("terminate");
}

#[test]
fn guardian_startup_defers_bootstrap_messages_until_after_setup() {
  let observed = ArcShared::new(SpinSyncMutex::new(Vec::<String>::new()));
  let observed_factory = observed.clone();
  let guardian_props = TypedProps::<u32>::from_behavior_factory(move || {
    let observed_setup = observed_factory.clone();
    Behaviors::setup(move |_ctx| {
      observed_setup.lock().push("start".to_string());
      let observed_receive = observed_setup.clone();
      Behaviors::receive_message(move |_ctx, message| {
        observed_receive.lock().push(format!("message:{message}"));
        Ok(Behaviors::same())
      })
    })
  });
  let config = ActorSystemConfig::new(crate::test_support::test_tick_driver());

  let system = TypedActorSystem::<u32>::create_from_props_with_init(&guardian_props, config, |system| {
    let mut guardian = system.user_guardian_ref();
    guardian
      .try_tell(AnyMessage::new(7_u32))
      .map_err(|error| SpawnError::invalid_props(format!("bootstrap guardian send failed: {error:?}")))?;
    Ok(())
  })
  .expect("system");

  wait_until(|| observed.lock().len() == 2);
  assert_eq!(*observed.lock(), vec!["start".to_string(), "message:7".to_string()]);
  system.terminate().expect("terminate");
}
