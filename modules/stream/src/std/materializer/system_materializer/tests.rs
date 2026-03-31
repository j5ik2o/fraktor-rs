use core::any::TypeId;

use fraktor_actor_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    extension::{ExtensionId, ExtensionInstallers},
    messaging::AnyMessageView,
    props::Props,
    scheduler::{
      SchedulerConfig,
      tick_driver::{ManualTestDriver, TickDriverConfig},
    },
    setup::ActorSystemConfig,
  },
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_utils_rs::core::sync::ArcShared;

use crate::{
  core::materialization::ActorMaterializer,
  std::materializer::{SystemMaterializer, SystemMaterializerId},
};

// --- test helpers ---

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfig::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);
  ActorSystem::new_with_config(&props, &config).expect("system should build")
}

fn build_system_with_materializer() -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let installers = ExtensionInstallers::default().with_extension_installer(
    |system: &ActorSystem| -> Result<(), ActorSystemBuildError> {
      system
        .extended()
        .register_extension(&SystemMaterializerId)
        .map_err(|e| ActorSystemBuildError::Configuration(alloc::format!("{:?}", e)))?;
      Ok(())
    },
  );
  let config = ActorSystemConfig::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_extension_installers(installers);
  ActorSystem::new_with_config(&props, &config).expect("system should build")
}

// --- SystemMaterializerId ---

#[test]
fn system_materializer_id_should_have_stable_type_id() {
  // Given: a SystemMaterializerId instance
  let id = SystemMaterializerId;

  // Then: its type ID should be deterministic
  assert_eq!(<SystemMaterializerId as ExtensionId>::id(&id), TypeId::of::<SystemMaterializerId>());
}

#[test]
fn system_materializer_id_should_create_extension() {
  // Given: an ActorSystem
  let system = build_system();

  // When: creating extension via factory
  let id = SystemMaterializerId;
  let ext = id.create_extension(&system);

  // Then: a SystemMaterializer should be created
  let _materializer: &ActorMaterializer = ext.materializer();

  system.terminate().expect("terminate");
}

// --- SystemMaterializer registration ---

#[test]
fn system_materializer_should_be_registerable_as_extension() {
  // Given: an ActorSystem with SystemMaterializer registered via ExtensionInstallers
  let system = build_system_with_materializer();

  // Then: the extension should be retrievable and hold a valid materializer
  let ext: ArcShared<SystemMaterializer> =
    system.extended().extension(&SystemMaterializerId).expect("extension should be registered");
  let _materializer = ext.materializer();

  system.terminate().expect("terminate");
}

#[test]
fn system_materializer_should_be_retrievable_after_registration() {
  // Given: an ActorSystem with SystemMaterializer registered
  let system = build_system_with_materializer();

  // When: looking up the extension
  let retrieved: Option<ArcShared<SystemMaterializer>> = system.extended().extension(&SystemMaterializerId);

  // Then: the extension should be found
  assert!(retrieved.is_some());

  system.terminate().expect("terminate");
}

#[test]
fn system_materializer_should_return_none_when_not_registered() {
  // Given: an ActorSystem without SystemMaterializer registered
  let system = build_system();

  // When: looking up the extension
  let result: Option<ArcShared<SystemMaterializer>> = system.extended().extension(&SystemMaterializerId);

  // Then: should return None
  assert!(result.is_none());

  system.terminate().expect("terminate");
}

#[test]
fn system_materializer_should_return_same_instance_on_repeated_lookup() {
  // Given: an ActorSystem with SystemMaterializer registered
  let system = build_system_with_materializer();

  // When: looking up the extension twice
  let first: ArcShared<SystemMaterializer> = system.extended().extension(&SystemMaterializerId).expect("first lookup");
  let second: ArcShared<SystemMaterializer> =
    system.extended().extension(&SystemMaterializerId).expect("second lookup");

  // Then: both lookups should return the same shared instance
  assert!(ArcShared::ptr_eq(&first, &second));

  system.terminate().expect("terminate");
}

// --- SystemMaterializer::materializer ---

#[test]
fn system_materializer_should_provide_working_materializer() {
  // Given: a SystemMaterializer registered with an ActorSystem
  let system = build_system_with_materializer();

  let ext: ArcShared<SystemMaterializer> =
    system.extended().extension(&SystemMaterializerId).expect("extension should be registered");

  // When/Then: the materializer reference should be valid
  let _materializer = ext.materializer();

  system.terminate().expect("terminate");
}

#[test]
fn system_materializer_should_provide_mutable_materializer() {
  // 準備: create_extension で SystemMaterializer を直接生成する。
  // NOTE: Extension は ArcShared の背後にあるため、内部の
  // SystemMaterializer への可変アクセスには以下のいずれかが必要:
  // (a) SharedAccess パターン (with_write)、または
  // (b) 単一の ArcShared を取得して try_unwrap / get_mut を使用。
  // このテストは materializer_mut() メソッドの存在と、
  // 排他アクセス時の安全性を検証する。
  let system = build_system();
  let id = SystemMaterializerId;
  let mut ext = id.create_extension(&system);

  // 実行: materializer への可変参照を取得
  let _materializer_mut = ext.materializer_mut();

  // 検証: 排他所有時に可変アクセスが可能であること
  system.terminate().expect("terminate");
}
