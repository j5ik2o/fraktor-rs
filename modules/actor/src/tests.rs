use alloc::boxed::Box;
use core::any::TypeId;
use std::path::Path;

use crate::core::kernel::actor::scheduler::tick_driver::{
  HardwareTickDriver, SchedulerTickExecutor, TickDriver, TickDriverBundle, TickDriverConfig, TickDriverControl,
  TickDriverError, TickDriverHandle, TickDriverId, TickDriverKind, TickDriverProvisioningContext, TickExecutorPump,
  TickExecutorSignal, TickFeed, TickPulseSource,
};

#[test]
fn deleted_std_tree_stays_deleted() {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let removed_paths = ["src/std.rs", "src/std"];

  for relative_path in removed_paths {
    let path = manifest_dir.join(relative_path);
    assert!(!path.exists(), "actor crate に削除済み std ツリーが復活しています: {}", path.display());
  }
}

#[test]
fn tick_driver_public_surface_keeps_primary_boundary_contracts() {
  let _driver: Option<Box<dyn TickDriver>> = None;
  let _pulse_source: Option<Box<dyn TickPulseSource>> = None;
  let _control: Option<Box<dyn TickDriverControl>> = None;
  let _pump: Option<Box<dyn TickExecutorPump>> = None;

  let _ = TypeId::of::<HardwareTickDriver>();
  let _ = TypeId::of::<SchedulerTickExecutor>();
  let _ = TypeId::of::<TickDriverBundle>();
  let _ = TypeId::of::<TickDriverConfig>();
  let _ = TypeId::of::<TickDriverError>();
  let _ = TypeId::of::<TickDriverHandle>();
  let _ = TypeId::of::<TickDriverId>();
  let _ = TypeId::of::<TickDriverKind>();
  let _ = TypeId::of::<TickDriverProvisioningContext>();
  let _ = TypeId::of::<TickExecutorSignal>();
  let _ = TypeId::of::<TickFeed>();
}

#[test]
fn tick_executor_pump_file_exists_for_core_runtime_wiring() {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let path = manifest_dir.join("src/core/kernel/actor/scheduler/tick_driver/tick_executor_pump.rs");

  assert!(path.exists(), "core runtime wiring 用の tick executor pump 契約ファイルが必要です: {}", path.display());
}

#[test]
fn tick_driver_factory_file_stays_deleted() {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let path = manifest_dir.join("src/core/kernel/actor/scheduler/tick_driver/tick_driver_factory.rs");

  assert!(!path.exists(), "tick driver factory の未使用実装が残っています: {}", path.display());
}

#[test]
fn tick_driver_module_stays_unexported_from_factory_surface() {
  let source = include_str!("core/kernel/actor/scheduler/tick_driver.rs");

  assert!(
    !source.contains("pub use tick_driver_factory::{TickDriverFactory, TickDriverFactoryRef};"),
    "tick driver factory は tick_driver 公開面から除外されている必要があります"
  );
}

#[test]
fn tick_driver_factory_module_wiring_stays_removed() {
  let source = include_str!("core/kernel/actor/scheduler/tick_driver.rs");

  assert!(
    !source.contains("mod tick_driver_factory;"),
    "tick driver factory モジュール配線は tick_driver から除去されている必要があります"
  );
}

#[test]
fn tick_executor_pump_module_is_wired_into_tick_driver_surface() {
  let source = include_str!("core/kernel/actor/scheduler/tick_driver.rs");

  assert!(source.contains("mod tick_executor_pump;"), "tick executor pump モジュール配線が tick_driver に必要です");
  assert!(
    source.contains("pub use tick_executor_pump::TickExecutorPump;"),
    "tick executor pump 契約が tick_driver 公開面に必要です"
  );
}
