//! Tests for tick driver configuration.

use alloc::boxed::Box;
use core::time::Duration;

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::core::kernel::actor::scheduler::tick_driver::{
  SchedulerTickExecutor, TickDriver, TickDriverConfig, TickDriverControl, TickDriverError, TickDriverHandle,
  TickDriverId, TickDriverKind, TickExecutorPump, TickFeedHandle,
};

struct NoopControl;

impl TickDriverControl for NoopControl {
  fn shutdown(&self) {}
}

struct RuntimeTestDriver;

impl TickDriver for RuntimeTestDriver {
  fn id(&self) -> TickDriverId {
    TickDriverId::new(1)
  }

  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Auto
  }

  fn resolution(&self) -> Duration {
    Duration::from_millis(1)
  }

  fn start(&mut self, _feed: TickFeedHandle) -> Result<TickDriverHandle, TickDriverError> {
    let control: Box<dyn TickDriverControl> = Box::new(NoopControl);
    let control = ArcShared::new(RuntimeMutex::new(control));
    Ok(TickDriverHandle::new(self.id(), self.kind(), self.resolution(), control))
  }
}

struct RuntimeTestPump;

impl TickExecutorPump for RuntimeTestPump {
  fn spawn(&mut self, _executor: SchedulerTickExecutor) -> Result<Box<dyn TickDriverControl>, TickDriverError> {
    Ok(Box::new(NoopControl))
  }
}

#[cfg(any(test, feature = "test-support"))]
#[test]
fn test_tick_driver_config_manual_test() {
  use crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver;

  let driver = ManualTestDriver::new();
  let config = TickDriverConfig::manual(driver);

  match config {
    | TickDriverConfig::ManualTest(_) => {},
    #[allow(unreachable_patterns)]
    | _ => panic!("expected ManualTest variant"),
  }
}

#[test]
fn test_tick_driver_config_declares_runtime_wiring_variant() {
  let config = TickDriverConfig::runtime(Box::new(RuntimeTestDriver), Box::new(RuntimeTestPump));

  match config {
    | TickDriverConfig::Runtime { .. } => {},
    #[allow(unreachable_patterns)]
    | _ => panic!("expected Runtime variant"),
  }
}
