//! Configured Phi Accrual detector bridge.

use core::time::Duration;

use fraktor_cluster_core_kernel_rs::failure_detector::{FailureDetector, FailureDetectorConfig};
use fraktor_remote_core_rs::{address::Address, failure_detector::PhiAccrualFailureDetector};

#[cfg(test)]
#[path = "configured_phi_accrual_detector_factory_test.rs"]
mod tests;

/// Creates Phi Accrual failure detectors from cluster failure detector configuration.
pub struct ConfiguredPhiAccrualDetectorFactory {
  config:            FailureDetectorConfig,
  monitored_address: Address,
}

impl ConfiguredPhiAccrualDetectorFactory {
  /// Creates a configured Phi Accrual detector factory.
  #[must_use]
  pub const fn new(config: FailureDetectorConfig, monitored_address: Address) -> Self {
    Self { config, monitored_address }
  }

  /// Creates a cluster-core failure detector.
  #[must_use]
  pub fn create(&self) -> Box<dyn FailureDetector + Send> {
    Box::new(PhiAccrualAdapter(self.create_phi_accrual_detector()))
  }

  fn create_phi_accrual_detector(&self) -> PhiAccrualFailureDetector {
    PhiAccrualFailureDetector::new(
      self.monitored_address.clone(),
      self.config.phi_threshold(),
      self.config.max_sample_size(),
      millis_u64(self.config.min_standard_deviation()),
      millis_u64(self.config.acceptable_heartbeat_pause()),
      millis_u64(self.config.first_heartbeat_estimate()),
    )
  }
}

struct PhiAccrualAdapter(PhiAccrualFailureDetector);

impl FailureDetector for PhiAccrualAdapter {
  fn is_available(&self, now_ms: u64) -> bool {
    self.0.is_available(now_ms)
  }

  fn is_monitoring(&self) -> bool {
    self.0.is_monitoring()
  }

  fn heartbeat(&mut self, now_ms: u64) {
    self.0.heartbeat(now_ms);
  }
}

fn millis_u64(value: Duration) -> u64 {
  u64::try_from(value.as_millis()).unwrap_or(u64::MAX)
}
