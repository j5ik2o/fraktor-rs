#![cfg(feature = "test-support")]

use fraktor_remote_rs::core::failure_detector::{
  DefaultFailureDetectorRegistry, FailureDetector, FailureDetectorRegistry,
  phi_failure_detector::{PhiFailureDetector, PhiFailureDetectorConfig},
};

fn detector_with(threshold: f64, max_sample_size: usize, minimum_interval_ms: u64) -> PhiFailureDetector {
  PhiFailureDetector::new(PhiFailureDetectorConfig::new(threshold, max_sample_size, minimum_interval_ms))
}

fn registry_with(
  threshold: f64,
  max_sample_size: usize,
  minimum_interval_ms: u64,
) -> DefaultFailureDetectorRegistry<String> {
  DefaultFailureDetectorRegistry::new(Box::new(move || {
    Box::new(PhiFailureDetector::new(PhiFailureDetectorConfig::new(threshold, max_sample_size, minimum_interval_ms)))
  }))
}

#[test]
fn single_detector_is_available_without_heartbeats() {
  let detector = detector_with(1.5, 8, 1);
  assert!(detector.is_available(100));
  assert!(!detector.is_monitoring());
}

#[test]
fn single_detector_becomes_unavailable() {
  let mut detector = detector_with(1.5, 8, 1);
  detector.heartbeat(0);
  detector.heartbeat(10);
  assert!(!detector.is_available(40));
}

#[test]
fn single_detector_recovers_after_heartbeat() {
  let mut detector = detector_with(1.5, 8, 1);
  detector.heartbeat(0);
  detector.heartbeat(10);
  assert!(!detector.is_available(40));
  detector.heartbeat(41);
  assert!(detector.is_available(41));
}

#[test]
fn registry_returns_available_for_unregistered() {
  let registry = registry_with(1.5, 8, 1);
  assert!(registry.is_available(&String::from("node-a:4100"), 100));
}

#[test]
fn registry_detects_unavailability() {
  let mut registry = registry_with(1.5, 8, 1);
  let node = String::from("node-a:4100");
  registry.heartbeat(&node, 0);
  registry.heartbeat(&node, 10);
  assert!(!registry.is_available(&node, 40));
}

#[test]
fn minimum_interval_clamps_fast_heartbeats() {
  let mut detector = detector_with(2.0, 8, 20);
  detector.heartbeat(0);
  detector.heartbeat(1);
  assert!(detector.is_available(40));
  assert!(!detector.is_available(41));
}

#[test]
fn registry_tracks_resources_independently() {
  let mut registry = registry_with(1.5, 8, 1);
  let a = String::from("node-a:4400");
  let b = String::from("node-b:4401");

  registry.heartbeat(&a, 0);
  registry.heartbeat(&a, 10);

  registry.heartbeat(&b, 0);
  registry.heartbeat(&b, 10);
  registry.heartbeat(&b, 38);

  assert!(!registry.is_available(&a, 40));
  assert!(registry.is_available(&b, 40));
}
