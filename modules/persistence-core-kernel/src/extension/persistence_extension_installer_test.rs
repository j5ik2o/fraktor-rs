use alloc::{boxed::Box, vec::Vec};
use core::any::{Any, TypeId};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    extension::{ExtensionInstaller, ExtensionInstallers},
    scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  serialization::{
    SerializationError, SerializationExtensionInstaller, SerializationExtensionShared, SerializationSetupBuilder,
    Serializer, SerializerId, default_serialization_setup,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess};

use crate::{
  config::PersistenceSettings,
  extension::{PersistenceExtensionInstaller, PersistenceExtensionShared},
  journal::{InMemoryJournal, JournalActorConfig},
  persistent::{AtomicWrite, PersistentRepr},
  serialization::{MESSAGE_SERIALIZER_ID, SnapshotPayload},
  snapshot::{InMemorySnapshotStore, SnapshotActorConfig},
};

struct DummySerializer {
  id: SerializerId,
}

impl DummySerializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for DummySerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, _message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    Ok(Vec::new())
  }

  fn from_binary(
    &self,
    _bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Ok(Box::new(()))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

fn persistence_installer() -> PersistenceExtensionInstaller<InMemoryJournal, InMemorySnapshotStore> {
  PersistenceExtensionInstaller::new(InMemoryJournal::new(), InMemorySnapshotStore::new())
}

fn build_system(installers: ExtensionInstallers) -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler)
    .with_extension_installers(installers);
  ActorSystem::create_with_noop_guardian(config).expect("system")
}

fn assert_persistence_serializers_registered(system: &ActorSystem) {
  let serialization =
    system.extended().extension_by_type::<SerializationExtensionShared>().expect("serialization extension");
  serialization.with_read(|extension| {
    let registry = extension.registry();
    assert_eq!(registry.binding_for(TypeId::of::<PersistentRepr>()), Some(MESSAGE_SERIALIZER_ID));
    assert_eq!(registry.binding_for(TypeId::of::<AtomicWrite>()), Some(MESSAGE_SERIALIZER_ID));
    assert!(registry.binding_for(TypeId::of::<SnapshotPayload>()).is_some());
  });
}

#[test]
fn dummy_serializer_round_trips_unit_payload() {
  let serializer = DummySerializer::new(SerializerId::try_from(100).expect("serializer id"));

  assert_eq!(serializer.to_binary(&()).expect("binary"), Vec::<u8>::new());
  assert!(serializer.from_binary(&[], None).expect("value").downcast_ref::<()>().is_some());
}

#[test]
fn installer_registers_persistence_extension() {
  let installers = ExtensionInstallers::default().with_extension_installer(persistence_installer());
  let system = build_system(installers);

  let extension = system.extended().extension_by_type::<PersistenceExtensionShared>();

  assert!(extension.is_some());
  assert_persistence_serializers_registered(&system);
}

#[test]
fn installer_registers_extension_with_explicit_settings() {
  let journal = InMemoryJournal::new();
  let snapshot_store = InMemorySnapshotStore::new();
  let settings = PersistenceSettings::default()
    .with_journal_actor_config(JournalActorConfig::new(2))
    .with_snapshot_actor_config(SnapshotActorConfig::new(3));
  let installer = PersistenceExtensionInstaller::new_with_settings(journal, snapshot_store, settings);
  let installers = ExtensionInstallers::default().with_extension_installer(installer);
  let system = build_system(installers);

  let extension = system.extended().extension_by_type::<PersistenceExtensionShared>();

  assert!(extension.is_some());
}

#[test]
fn installer_composes_with_custom_serialization_before_persistence() {
  let serialization = SerializationExtensionInstaller::new(default_serialization_setup());
  let installers = ExtensionInstallers::default()
    .with_extension_installer(serialization)
    .with_extension_installer(persistence_installer());

  let system = build_system(installers);

  assert_persistence_serializers_registered(&system);
}

#[test]
fn installer_composes_with_custom_serialization_after_persistence() {
  let serialization = SerializationExtensionInstaller::new(default_serialization_setup());
  let installers = ExtensionInstallers::default()
    .with_extension_installer(persistence_installer())
    .with_extension_installer(serialization);

  let system = build_system(installers);

  assert_persistence_serializers_registered(&system);
}

#[test]
#[should_panic(expected = "failed to apply serialization registry contributors")]
fn installer_rejects_persistence_serializer_id_collision() {
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(DummySerializer::new(MESSAGE_SERIALIZER_ID));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("dummy", MESSAGE_SERIALIZER_ID, serializer)
    .expect("register")
    .set_fallback("dummy")
    .expect("fallback")
    .build()
    .expect("setup");
  let serialization = SerializationExtensionInstaller::new(setup);
  let installers = ExtensionInstallers::default()
    .with_extension_installer(persistence_installer())
    .with_extension_installer(serialization);

  let _system = build_system(installers);
}

#[test]
fn installer_reports_persistence_serializer_registration_error() {
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(DummySerializer::new(MESSAGE_SERIALIZER_ID));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("dummy", MESSAGE_SERIALIZER_ID, serializer)
    .expect("register")
    .set_fallback("dummy")
    .expect("fallback")
    .build()
    .expect("setup");
  let serialization = SerializationExtensionInstaller::new(setup);
  let system = build_system(ExtensionInstallers::default().with_extension_installer(serialization));

  let result = persistence_installer().install(&system);

  assert!(matches!(result, Err(error) if error.to_string().contains("persistence serialization registration failed")));
}

#[test]
#[should_panic(expected = "failed to apply serialization registry contributors")]
fn installer_rejects_persistence_type_binding_collision() {
  let dummy_id = SerializerId::try_from(100).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(DummySerializer::new(dummy_id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("dummy", dummy_id, serializer)
    .expect("register")
    .set_fallback("dummy")
    .expect("fallback")
    .bind::<PersistentRepr>("dummy")
    .expect("bind")
    .build()
    .expect("setup");
  let serialization = SerializationExtensionInstaller::new(setup);
  let installers = ExtensionInstallers::default()
    .with_extension_installer(persistence_installer())
    .with_extension_installer(serialization);

  let _system = build_system(installers);
}
