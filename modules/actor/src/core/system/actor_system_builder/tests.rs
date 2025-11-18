use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use super::ActorSystemBuilder;
use crate::core::{
  actor_prim::{Actor, ActorContextGeneric},
  error::ActorError,
  extension::ExtensionsConfig,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  system::{ActorRefProviderInstaller, ActorSystemBuildError},
};

struct NoopActor;

impl Actor<NoStdToolbox> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[derive(Clone)]
struct TestProvider {
  marker: usize,
}

struct TestProviderInstaller {
  marker: usize,
}

impl ActorRefProviderInstaller<NoStdToolbox> for TestProviderInstaller {
  fn install(
    &self,
    system: &crate::core::system::ActorSystemGeneric<NoStdToolbox>,
  ) -> Result<(), ActorSystemBuildError> {
    system.register_actor_ref_provider(ArcShared::new(TestProvider { marker: self.marker }));
    Ok(())
  }
}

struct TestExtensionInstaller {
  counter: ArcShared<AtomicUsize>,
}

impl crate::core::extension::ExtensionInstaller<NoStdToolbox> for TestExtensionInstaller {
  fn install(
    &self,
    _system: &crate::core::system::ActorSystemGeneric<NoStdToolbox>,
  ) -> Result<(), ActorSystemBuildError> {
    self.counter.fetch_add(1, Ordering::SeqCst);
    Ok(())
  }
}

#[test]
fn installs_extensions_and_provider() {
  let counter = ArcShared::new(AtomicUsize::new(0));
  let extensions =
    ExtensionsConfig::default().with_extension_config(TestExtensionInstaller { counter: counter.clone() });

  let props = PropsGeneric::from_fn(|| NoopActor).with_name("quickstart");
  let system = ActorSystemBuilder::<NoStdToolbox>::new(props)
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()))
    .with_extensions_config(extensions)
    .with_actor_ref_provider(TestProviderInstaller { marker: 7 })
    .build()
    .expect("builder");

  assert_eq!(counter.load(Ordering::SeqCst), 1);
  let provider = system.actor_ref_provider::<TestProvider>().expect("provider registered");
  assert_eq!(provider.marker, 7);
}
