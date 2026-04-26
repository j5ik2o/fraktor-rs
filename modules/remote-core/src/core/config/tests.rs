use core::time::Duration;

use crate::core::config::RemoteConfig;

#[test]
fn new_uses_defaults_for_optional_fields() {
  let s = RemoteConfig::new("localhost");
  assert_eq!(s.canonical_host(), "localhost");
  assert_eq!(s.canonical_port(), None);
  assert_eq!(s.handshake_timeout(), Duration::from_secs(15));
  assert_eq!(s.shutdown_flush_timeout(), Duration::from_secs(5));
  assert!(s.flight_recorder_capacity() > 0);
}

#[test]
fn with_canonical_port_sets_some() {
  let s = RemoteConfig::new("localhost").with_canonical_port(8080);
  assert_eq!(s.canonical_port(), Some(8080));
}

#[test]
fn method_chain_applies_all_changes() {
  let s = RemoteConfig::new("localhost")
    .with_canonical_port(8080)
    .with_handshake_timeout(Duration::from_secs(30))
    .with_shutdown_flush_timeout(Duration::from_secs(10))
    .with_flight_recorder_capacity(4096);
  assert_eq!(s.canonical_host(), "localhost");
  assert_eq!(s.canonical_port(), Some(8080));
  assert_eq!(s.handshake_timeout(), Duration::from_secs(30));
  assert_eq!(s.shutdown_flush_timeout(), Duration::from_secs(10));
  assert_eq!(s.flight_recorder_capacity(), 4096);
}

#[test]
fn cloning_preserves_immutability_of_original() {
  let a = RemoteConfig::new("localhost");
  let b = a.clone().with_canonical_port(8080);
  assert_eq!(a.canonical_port(), None);
  assert_eq!(b.canonical_port(), Some(8080));
}

#[test]
fn equality_and_clone_are_consistent() {
  let a = RemoteConfig::new("localhost").with_canonical_port(1234);
  let b = a.clone();
  assert_eq!(a, b);
}

#[test]
fn with_flight_recorder_capacity_respects_input() {
  let s = RemoteConfig::new("h").with_flight_recorder_capacity(1);
  assert_eq!(s.flight_recorder_capacity(), 1);
}
