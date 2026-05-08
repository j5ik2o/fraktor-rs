use core::{
  any::TypeId,
  sync::atomic::{AtomicUsize, Ordering},
};

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{Extension, ExtensionId, ExtensionInstaller, ExtensionInstallers};
use crate::core::kernel::{
  actor::{scheduler::tick_driver::tests::TestTickDriver, setup::ActorSystemConfig},
  system::{ActorSystem, ActorSystemBuildError},
};

struct DummyExt;
impl Extension for DummyExt {}

struct DummyId;

impl ExtensionId for DummyId {
  type Ext = DummyExt;

  fn create_extension(&self, _system: &ActorSystem) -> Self::Ext {
    DummyExt
  }
}

#[test]
fn type_id_is_stable() {
  let id = DummyId;
  assert_eq!(<DummyId as ExtensionId>::id(&id), TypeId::of::<DummyId>());
}

#[test]
fn factory_creates_extension() {
  let system = ActorSystem::new_empty();
  let id = DummyId;
  let ext = id.create_extension(&system);
  let _shared: ArcShared<DummyExt> = ArcShared::new(ext);
}

struct CountingInstaller {
  calls: ArcShared<AtomicUsize>,
}

impl CountingInstaller {
  fn new(calls: ArcShared<AtomicUsize>) -> Self {
    Self { calls }
  }
}

impl ExtensionInstaller for CountingInstaller {
  fn install(&self, _system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    self.calls.fetch_add(1, Ordering::Relaxed);
    Ok(())
  }
}

#[test]
fn shared_extension_installer_keeps_caller_handle_usable() {
  let calls = ArcShared::new(AtomicUsize::new(0));
  let installer = ArcShared::new(CountingInstaller::new(calls.clone()));
  let installers = ExtensionInstallers::default().with_shared_extension_installer(installer.clone());
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_extension_installers(installers);

  let system = ActorSystem::create_with_noop_guardian(config).expect("system should build");

  assert_eq!(calls.load(Ordering::Relaxed), 1);
  assert_eq!(installer.calls.load(Ordering::Relaxed), 1);
  system.terminate().expect("terminate");
}
