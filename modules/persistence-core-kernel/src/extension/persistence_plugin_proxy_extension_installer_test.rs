use std::panic::{AssertUnwindSafe, catch_unwind};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    extension::{ExtensionId, ExtensionInstallers},
    scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  serialization::{SerializationExtensionInstaller, SerializationSetupBuilder},
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, WeakShared};

use crate::{
  config::PersistenceConfig,
  extension::{
    PersistenceExtensionShared, PersistencePluginProxyExtensionId, PersistencePluginProxyExtensionInstaller,
  },
  journal::JournalActorConfig,
  serialization::{MESSAGE_SERIALIZER_ID, SnapshotSerializer},
  snapshot::SnapshotActorConfig,
};

fn build_system(installers: ExtensionInstallers) -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler)
    .with_extension_installers(installers);
  ActorSystem::create_with_noop_guardian(config).expect("system")
}

#[test]
fn proxy_installer_registers_persistence_extension() {
  let installers =
    ExtensionInstallers::default().with_extension_installer(PersistencePluginProxyExtensionInstaller::new());
  let system = build_system(installers);

  let extension = system.extended().extension_by_type::<PersistenceExtensionShared>();

  assert!(extension.is_some());
}

#[test]
fn proxy_installer_default_registers_persistence_extension() {
  let installers =
    ExtensionInstallers::default().with_extension_installer(PersistencePluginProxyExtensionInstaller::default());
  let system = build_system(installers);

  assert!(system.extended().extension_by_type::<PersistenceExtensionShared>().is_some());
}

#[test]
fn proxy_extension_id_default_creates_proxy_extension() {
  let system = build_system(ExtensionInstallers::default());
  let extension_id = PersistencePluginProxyExtensionId::default();

  let extension = extension_id.create_extension(&system);

  extension.with_read(|inner| {
    assert_eq!(inner.settings(), PersistenceConfig::default());
  });
}

#[test]
fn proxy_extension_id_panics_when_proxy_actor_names_are_already_taken() {
  let system = build_system(ExtensionInstallers::default());
  let extension_id = PersistencePluginProxyExtensionId::new();
  let _extension = extension_id.create_extension(&system);

  let result = catch_unwind(AssertUnwindSafe(|| {
    let _duplicate = extension_id.create_extension(&system);
  }));

  assert!(result.is_err());
}

#[test]
fn proxy_installer_reports_serialization_registration_failure() {
  let conflicting_serializer = SnapshotSerializer::new(MESSAGE_SERIALIZER_ID, WeakShared::new());
  let serializer = ArcShared::new(conflicting_serializer);
  let setup = SerializationSetupBuilder::new()
    .register_serializer("conflicting-persistence", MESSAGE_SERIALIZER_ID, serializer)
    .expect("register serializer")
    .set_fallback("conflicting-persistence")
    .expect("set fallback")
    .build()
    .expect("build setup");
  let installers = ExtensionInstallers::default()
    .with_extension_installer(SerializationExtensionInstaller::new(setup))
    .with_extension_installer(PersistencePluginProxyExtensionInstaller::new());
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler)
    .with_extension_installers(installers);

  let result = ActorSystem::create_with_noop_guardian(config);

  assert!(result.is_err());
}

#[test]
fn proxy_installer_preserves_explicit_settings() {
  let settings = PersistenceConfig::default()
    .with_journal_actor_config(JournalActorConfig::new(2))
    .with_snapshot_actor_config(SnapshotActorConfig::new(3));
  let installers = ExtensionInstallers::default()
    .with_extension_installer(PersistencePluginProxyExtensionInstaller::new_with_settings(settings));
  let system = build_system(installers);
  let extension = system.extended().extension_by_type::<PersistenceExtensionShared>().expect("extension");

  extension.with_read(|inner| {
    assert_eq!(inner.settings().journal_actor_config(), JournalActorConfig::new(2));
    assert_eq!(inner.settings().snapshot_actor_config(), SnapshotActorConfig::new(3));
  });
}
