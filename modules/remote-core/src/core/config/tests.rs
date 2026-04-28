use core::time::Duration;

use crate::core::config::RemoteConfig;

const DEFAULT_MAXIMUM_FRAME_SIZE: usize = 256 * 1024;
const DEFAULT_BUFFER_POOL_SIZE: usize = 128;
const DEFAULT_OUTBOUND_MESSAGE_QUEUE_SIZE: usize = 3072;
const DEFAULT_OUTBOUND_CONTROL_QUEUE_SIZE: usize = 20_000;
const DEFAULT_REMOVE_QUARANTINED_ASSOCIATION_AFTER: Duration = Duration::from_secs(60 * 60);
const MINIMUM_MAXIMUM_FRAME_SIZE: usize = 32 * 1024;

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
fn advanced_artery_settings_use_pekko_compatible_defaults() {
  // Given: デフォルト構成
  let s = RemoteConfig::new("localhost");

  // Then: Pekko Artery 相当の既定値を返す
  assert_eq!(s.bind_hostname(), None);
  assert_eq!(s.bind_port(), None);
  assert_eq!(s.inbound_lanes(), 4);
  assert_eq!(s.outbound_lanes(), 1);
  assert_eq!(s.maximum_frame_size(), DEFAULT_MAXIMUM_FRAME_SIZE);
  assert_eq!(s.buffer_pool_size(), DEFAULT_BUFFER_POOL_SIZE);
  assert_eq!(s.outbound_message_queue_size(), DEFAULT_OUTBOUND_MESSAGE_QUEUE_SIZE);
  assert_eq!(s.outbound_control_queue_size(), DEFAULT_OUTBOUND_CONTROL_QUEUE_SIZE);
  assert_eq!(s.remove_quarantined_association_after(), DEFAULT_REMOVE_QUARANTINED_ASSOCIATION_AFTER);
  assert!(!s.untrusted_mode());
  assert!(!s.log_received_messages());
  assert!(!s.log_sent_messages());
  assert_eq!(s.log_frame_size_exceeding(), None);
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
fn advanced_artery_settings_method_chain_applies_all_changes() {
  // Given: advanced 設定をすべて上書きした構成
  let s = RemoteConfig::new("localhost")
    .with_bind_hostname("0.0.0.0")
    .with_bind_port(25520)
    .with_inbound_lanes(8)
    .with_outbound_lanes(2)
    .with_maximum_frame_size(512 * 1024)
    .with_buffer_pool_size(64)
    .with_outbound_message_queue_size(32)
    .with_outbound_control_queue_size(8)
    .with_remove_quarantined_association_after(Duration::from_secs(30))
    .with_untrusted_mode(true)
    .with_log_received_messages(true)
    .with_log_sent_messages(true)
    .with_log_frame_size_exceeding(128 * 1024);

  // Then: 上書きした値を保持する
  assert_eq!(s.bind_hostname(), Some("0.0.0.0"));
  assert_eq!(s.bind_port(), Some(25520));
  assert_eq!(s.inbound_lanes(), 8);
  assert_eq!(s.outbound_lanes(), 2);
  assert_eq!(s.maximum_frame_size(), 512 * 1024);
  assert_eq!(s.buffer_pool_size(), 64);
  assert_eq!(s.outbound_message_queue_size(), 32);
  assert_eq!(s.outbound_control_queue_size(), 8);
  assert_eq!(s.remove_quarantined_association_after(), Duration::from_secs(30));
  assert!(s.untrusted_mode());
  assert!(s.log_received_messages());
  assert!(s.log_sent_messages());
  assert_eq!(s.log_frame_size_exceeding(), Some(128 * 1024));
}

#[test]
fn with_inbound_lanes_rejects_zero() {
  // When: inbound lane に 0 を指定する
  let result = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_inbound_lanes(0));

  // Then: 不正な lane 数として拒否する
  assert!(result.is_err());
}

#[test]
fn with_outbound_lanes_rejects_zero() {
  // When: outbound lane に 0 を指定する
  let result = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_lanes(0));

  // Then: 不正な lane 数として拒否する
  assert!(result.is_err());
}

#[test]
fn with_maximum_frame_size_rejects_values_below_minimum() {
  // When: 最小値未満の frame size を指定する
  let result =
    std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_maximum_frame_size(MINIMUM_MAXIMUM_FRAME_SIZE - 1));

  // Then: 不正な frame size として拒否する
  assert!(result.is_err());
}

#[test]
fn with_buffer_pool_size_rejects_zero() {
  // When: buffer pool size に 0 を指定する
  let result = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_buffer_pool_size(0));

  // Then: 不正な pool size として拒否する
  assert!(result.is_err());
}

#[test]
fn with_outbound_message_queue_size_rejects_zero() {
  // When: outbound message queue size に 0 を指定する
  let result = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_message_queue_size(0));

  // Then: 不正な queue size として拒否する
  assert!(result.is_err());
}

#[test]
fn with_outbound_control_queue_size_rejects_zero() {
  // When: outbound control queue size に 0 を指定する
  let result = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_control_queue_size(0));

  // Then: 不正な queue size として拒否する
  assert!(result.is_err());
}

#[test]
fn with_remove_quarantined_association_after_rejects_zero() {
  // When: remove quarantined association after に 0 を指定する
  let result = std::panic::catch_unwind(|| {
    RemoteConfig::new("localhost").with_remove_quarantined_association_after(Duration::ZERO)
  });

  // Then: 不正な duration として拒否する
  assert!(result.is_err());
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
