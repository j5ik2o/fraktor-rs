//! Tests for tick driver configuration.

use crate::{NoStdToolbox, scheduler::TickDriverConfig};

#[cfg(any(test, feature = "test-support"))]
#[test]
fn test_tick_driver_config_manual_test() {
  use crate::scheduler::ManualTestDriver;

  let driver = ManualTestDriver::<NoStdToolbox>::new();
  let config = TickDriverConfig::manual(driver);

  match config {
    | TickDriverConfig::ManualTest(_) => {},
    #[allow(unreachable_patterns)]
    | _ => panic!("expected ManualTest variant"),
  }
}

#[test]
fn test_tick_driver_config_builder() {
  use crate::scheduler::TickDriverError;

  let config = TickDriverConfig::<NoStdToolbox>::new(|_ctx| {
    // Dummy builder for testing
    Err(TickDriverError::UnsupportedEnvironment)
  });

  match config {
    | TickDriverConfig::Builder { .. } => {},
    #[allow(unreachable_patterns)]
    | _ => panic!("expected Builder variant"),
  }
}
