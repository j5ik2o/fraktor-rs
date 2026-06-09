use core::time::Duration;

use fraktor_cluster_core_kernel_rs::failure_detector::{FailureDetector, FailureDetectorConfig};
use fraktor_remote_core_rs::address::Address;

use super::ConfiguredPhiAccrualDetectorFactory;

fn address() -> Address {
  Address::new("cluster-test", "127.0.0.1", 2552)
}

#[test]
fn creates_phi_accrual_detector_from_failure_detector_config() {
  let config = FailureDetectorConfig::new()
    .with_phi_threshold(8.5)
    .with_max_sample_size(64)
    .with_min_standard_deviation(Duration::from_millis(250))
    .with_acceptable_heartbeat_pause(Duration::from_secs(3))
    .with_first_heartbeat_estimate(Duration::from_millis(750));
  let factory = ConfiguredPhiAccrualDetectorFactory::new(config, address());

  let detector = factory.create_phi_accrual_detector();

  assert_eq!(detector.threshold(), 8.5);
  assert_eq!(detector.max_sample_size(), 64);
  assert_eq!(detector.min_std_deviation(), 250);
  assert_eq!(detector.acceptable_heartbeat_pause(), 3_000);
  assert_eq!(detector.first_heartbeat_estimate(), 750);
}

#[test]
fn creates_phi_accrual_detector_without_truncating_sub_millisecond_durations_to_zero() {
  let config = FailureDetectorConfig::new()
    .with_min_standard_deviation(Duration::from_nanos(1))
    .with_acceptable_heartbeat_pause(Duration::from_nanos(1))
    .with_first_heartbeat_estimate(Duration::from_nanos(1));
  let factory = ConfiguredPhiAccrualDetectorFactory::new(config, address());

  let detector = factory.create_phi_accrual_detector();

  assert_eq!(detector.min_std_deviation(), 1);
  assert_eq!(detector.acceptable_heartbeat_pause(), 1);
  assert_eq!(detector.first_heartbeat_estimate(), 1);
}

#[test]
fn create_returns_cluster_core_failure_detector_trait_object() {
  let factory = ConfiguredPhiAccrualDetectorFactory::new(FailureDetectorConfig::new(), address());

  let mut detector: Box<dyn FailureDetector + Send> = factory.create();

  assert!(detector.is_available(0));
  assert!(!detector.is_monitoring());
  detector.heartbeat(100);
  assert!(detector.is_monitoring());
}
