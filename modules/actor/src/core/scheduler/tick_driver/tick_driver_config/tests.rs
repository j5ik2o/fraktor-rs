//! Tests for tick driver configuration.

use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::scheduler::TickDriverConfig;

#[cfg(any(test, feature = "test-support"))]
#[test]
fn test_tick_driver_config_manual_test() {
  use crate::core::scheduler::ManualTestDriver;

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
  use crate::core::scheduler::TickDriverError;

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
