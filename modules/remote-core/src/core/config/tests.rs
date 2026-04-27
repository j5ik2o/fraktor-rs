use core::time::Duration;

use crate::core::config::RemoteConfig;

#[test]
fn new_uses_defaults_for_optional_fields() {
  let s = RemoteConfig::new("localhost");

  assert_eq!(s.canonical_host(), "localhost");
  assert_eq!(s.canonical_port(), None);
  assert_eq!(s.handshake_timeout(), Duration::from_secs(20));
  assert_eq!(s.shutdown_flush_timeout(), Duration::from_secs(5));
  assert!(s.flight_recorder_capacity() > 0);
  assert_eq!(s.ack_send_window(), 1024);
  assert_eq!(s.ack_receive_window(), 1024);
  assert_eq!(s.system_message_buffer_size(), 20_000);
  assert_eq!(s.system_message_resend_interval(), Duration::from_secs(1));
  assert_eq!(s.give_up_system_message_after(), Duration::from_secs(6 * 60 * 60));
  assert_eq!(s.handshake_retry_interval(), Duration::from_secs(1));
  assert_eq!(s.inject_handshake_interval(), Duration::from_secs(1));
  assert_eq!(s.stop_idle_outbound_after(), Duration::from_secs(5 * 60));
  assert_eq!(s.quarantine_idle_outbound_after(), Duration::from_secs(6 * 60 * 60));
  assert_eq!(s.stop_quarantined_after_idle(), Duration::from_secs(3));
  assert_eq!(s.outbound_restart_backoff(), Duration::from_secs(1));
  assert_eq!(s.outbound_restart_timeout(), Duration::from_secs(5));
  assert_eq!(s.outbound_max_restarts(), 5);
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
    .with_flight_recorder_capacity(4096)
    .with_ack_send_window(128)
    .with_ack_receive_window(256)
    .with_system_message_buffer_size(64)
    .with_system_message_resend_interval(Duration::from_millis(250))
    .with_give_up_system_message_after(Duration::from_secs(60))
    .with_handshake_retry_interval(Duration::from_millis(500))
    .with_inject_handshake_interval(Duration::from_millis(750))
    .with_stop_idle_outbound_after(Duration::from_secs(11))
    .with_quarantine_idle_outbound_after(Duration::from_secs(12))
    .with_stop_quarantined_after_idle(Duration::from_secs(13))
    .with_outbound_restart_backoff(Duration::from_millis(100))
    .with_outbound_restart_timeout(Duration::from_millis(500))
    .with_outbound_max_restarts(2);

  assert_eq!(s.canonical_host(), "localhost");
  assert_eq!(s.canonical_port(), Some(8080));
  assert_eq!(s.handshake_timeout(), Duration::from_secs(30));
  assert_eq!(s.shutdown_flush_timeout(), Duration::from_secs(10));
  assert_eq!(s.flight_recorder_capacity(), 4096);
  assert_eq!(s.ack_send_window(), 128);
  assert_eq!(s.ack_receive_window(), 256);
  assert_eq!(s.system_message_buffer_size(), 64);
  assert_eq!(s.system_message_resend_interval(), Duration::from_millis(250));
  assert_eq!(s.give_up_system_message_after(), Duration::from_secs(60));
  assert_eq!(s.handshake_retry_interval(), Duration::from_millis(500));
  assert_eq!(s.inject_handshake_interval(), Duration::from_millis(750));
  assert_eq!(s.stop_idle_outbound_after(), Duration::from_secs(11));
  assert_eq!(s.quarantine_idle_outbound_after(), Duration::from_secs(12));
  assert_eq!(s.stop_quarantined_after_idle(), Duration::from_secs(13));
  assert_eq!(s.outbound_restart_backoff(), Duration::from_millis(100));
  assert_eq!(s.outbound_restart_timeout(), Duration::from_millis(500));
  assert_eq!(s.outbound_max_restarts(), 2);
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
