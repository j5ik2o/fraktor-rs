use alloc::{boxed::Box, string::String};

use fraktor_remote_core_rs::domain::failure_detector::PhiAccrualFailureDetector;

use super::DefaultFailureDetectorRegistry;
use crate::core::failure_detector::{FailureDetector, FailureDetectorRegistry};

/// Test-only adapter that bridges the remote-core detector to the
/// cluster-core `FailureDetector` trait.
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

fn registry() -> DefaultFailureDetectorRegistry<String> {
  DefaultFailureDetectorRegistry::new(Box::new(|| {
    Box::new(PhiAccrualAdapter(PhiAccrualFailureDetector::new(1.5, 4, 1, 0, 10)))
  }))
}

#[test]
fn should_return_available_for_unknown_resource() {
  let reg = registry();
  assert!(reg.is_available(&String::from("node-a"), 100));
}

#[test]
fn should_not_monitor_unknown_resource() {
  let reg = registry();
  assert!(!reg.is_monitoring(&String::from("node-a")));
}

#[test]
fn should_monitor_after_heartbeat() {
  let mut reg = registry();
  let key = String::from("node-a");
  reg.heartbeat(&key, 0);
  assert!(reg.is_monitoring(&key));
}

#[test]
fn should_detect_unavailability() {
  let mut reg = registry();
  let key = String::from("node-a");
  reg.heartbeat(&key, 0);
  reg.heartbeat(&key, 10);
  assert!(reg.is_available(&key, 10));
  assert!(!reg.is_available(&key, 40));
}

#[test]
fn should_track_resources_independently() {
  let mut reg = registry();
  let a = String::from("node-a");
  let b = String::from("node-b");

  reg.heartbeat(&a, 0);
  reg.heartbeat(&a, 10);

  reg.heartbeat(&b, 0);
  reg.heartbeat(&b, 10);
  reg.heartbeat(&b, 38);

  assert!(!reg.is_available(&a, 40));
  assert!(reg.is_available(&b, 40));
}

#[test]
fn should_remove_resource() {
  let mut reg = registry();
  let key = String::from("node-a");
  reg.heartbeat(&key, 0);
  assert!(reg.is_monitoring(&key));
  reg.remove(&key);
  assert!(!reg.is_monitoring(&key));
  assert!(reg.is_available(&key, 100));
}

#[test]
fn should_reset_all_resources() {
  let mut reg = registry();
  let a = String::from("node-a");
  let b = String::from("node-b");
  reg.heartbeat(&a, 0);
  reg.heartbeat(&b, 0);
  assert!(reg.is_monitoring(&a));
  assert!(reg.is_monitoring(&b));
  reg.reset();
  assert!(!reg.is_monitoring(&a));
  assert!(!reg.is_monitoring(&b));
}
