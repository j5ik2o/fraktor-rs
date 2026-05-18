use core::{num::NonZeroUsize, time::Duration};

use super::DispatcherConfig;

fn nz(value: usize) -> NonZeroUsize {
  NonZeroUsize::new(value).expect("non-zero")
}

#[test]
fn new_stores_all_fields() {
  let settings = DispatcherConfig::new("test-id", nz(5), Some(Duration::from_millis(50)), Duration::from_secs(2));
  assert_eq!(settings.id(), "test-id");
  assert_eq!(settings.throughput(), nz(5));
  assert_eq!(settings.throughput_deadline(), Some(Duration::from_millis(50)));
  assert_eq!(settings.shutdown_timeout(), Duration::from_secs(2));
}

#[test]
fn with_throughput_replaces_value() {
  let settings = DispatcherConfig::new("id", nz(1), None, Duration::from_secs(1)).with_throughput(nz(10));
  assert_eq!(settings.throughput(), nz(10));
}

#[test]
fn with_throughput_deadline_replaces_value() {
  let settings = DispatcherConfig::new("id", nz(1), None, Duration::from_secs(1))
    .with_throughput_deadline(Some(Duration::from_secs(3)));
  assert_eq!(settings.throughput_deadline(), Some(Duration::from_secs(3)));
}

#[test]
fn with_shutdown_timeout_replaces_value() {
  let settings =
    DispatcherConfig::new("id", nz(1), None, Duration::from_secs(1)).with_shutdown_timeout(Duration::from_secs(5));
  assert_eq!(settings.shutdown_timeout(), Duration::from_secs(5));
}

#[test]
fn with_defaults_uses_documented_constants() {
  let settings = DispatcherConfig::with_defaults("default-id");
  assert_eq!(settings.id(), "default-id");
  assert_eq!(settings.throughput(), nz(5));
  assert_eq!(settings.throughput_deadline(), None);
  assert_eq!(settings.shutdown_timeout(), Duration::from_secs(1));
}

#[test]
fn clone_preserves_all_fields() {
  let settings = DispatcherConfig::new("clone-me", nz(7), Some(Duration::from_millis(100)), Duration::from_secs(4));
  let cloned = settings.clone();
  assert_eq!(cloned.id(), settings.id());
  assert_eq!(cloned.throughput(), settings.throughput());
  assert_eq!(cloned.throughput_deadline(), settings.throughput_deadline());
  assert_eq!(cloned.shutdown_timeout(), settings.shutdown_timeout());
}
