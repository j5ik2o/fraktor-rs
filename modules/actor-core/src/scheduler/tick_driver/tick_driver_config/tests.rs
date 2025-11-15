//! Tests for tick driver configuration.

use core::time::Duration;

use crate::{
  NoStdToolbox,
  scheduler::{AutoDriverConfig, FallbackPolicy, TickDriverConfig, TickMetricsMode},
};

#[test]
fn test_tick_driver_config_auto_default() {
  let config = TickDriverConfig::<NoStdToolbox>::auto();
  match config {
    | TickDriverConfig::Auto(auto_config) => {
      assert!(auto_config.factory().is_none());
      assert!(auto_config.locator().is_none());
      assert!(auto_config.resolution().is_none());
      assert_eq!(*auto_config.metrics_mode(), TickMetricsMode::default());
      assert_eq!(*auto_config.fallback_policy(), FallbackPolicy::default());
    },
    | _ => panic!("expected Auto variant"),
  }
}

#[test]
fn test_auto_driver_config_with_resolution() {
  let resolution = Duration::from_millis(10);
  let config = AutoDriverConfig::<NoStdToolbox>::new().with_resolution(resolution);

  assert_eq!(config.resolution(), Some(resolution));
}

#[test]
fn test_auto_driver_config_with_fallback() {
  let policy = FallbackPolicy::FailFast;
  let config = AutoDriverConfig::<NoStdToolbox>::new().with_fallback(policy.clone());

  assert_eq!(*config.fallback_policy(), policy);
}

#[test]
fn test_auto_driver_config_with_metrics_mode() {
  let mode = TickMetricsMode::OnDemand;
  let config = AutoDriverConfig::<NoStdToolbox>::new().with_metrics_mode(mode.clone());

  assert_eq!(*config.metrics_mode(), mode);
}

#[test]
fn test_tick_driver_config_fluent_api() {
  let config = TickDriverConfig::<NoStdToolbox>::auto()
    .with_resolution(Duration::from_millis(10))
    .with_fallback(FallbackPolicy::FailFast)
    .with_metrics_mode(TickMetricsMode::OnDemand);

  match config {
    | TickDriverConfig::Auto(auto_config) => {
      assert_eq!(auto_config.resolution(), Some(Duration::from_millis(10)));
      assert_eq!(*auto_config.fallback_policy(), FallbackPolicy::FailFast);
      assert_eq!(*auto_config.metrics_mode(), TickMetricsMode::OnDemand);
    },
    | _ => panic!("expected Auto variant"),
  }
}

#[cfg(any(test, feature = "test-support"))]
#[test]
fn test_tick_driver_config_manual_test() {
  use crate::scheduler::ManualTestDriver;

  let driver = ManualTestDriver::<NoStdToolbox>::new();
  let config = TickDriverConfig::manual(driver);

  match config {
    | TickDriverConfig::ManualTest(_) => {},
    | _ => panic!("expected ManualTest variant"),
  }
}

#[test]
fn test_fallback_policy_default() {
  let policy = FallbackPolicy::default();
  match policy {
    | FallbackPolicy::Retry { attempts, backoff } => {
      assert_eq!(attempts, 3);
      assert_eq!(backoff, Duration::from_millis(50));
    },
    | _ => panic!("expected Retry variant"),
  }
}

#[test]
fn test_tick_metrics_mode_default() {
  let mode = TickMetricsMode::default();
  match mode {
    | TickMetricsMode::AutoPublish { interval } => {
      assert_eq!(interval, Duration::from_secs(1));
    },
    | _ => panic!("expected AutoPublish variant"),
  }
}
