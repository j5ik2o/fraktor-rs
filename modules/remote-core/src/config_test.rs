use core::{num::NonZeroUsize, time::Duration};

use crate::config::{LargeMessageDestinationPattern, LargeMessageDestinations, RemoteCompressionConfig, RemoteConfig};

const DEFAULT_MAXIMUM_FRAME_SIZE: usize = 256 * 1024;
const DEFAULT_BUFFER_POOL_SIZE: usize = 128;
const DEFAULT_OUTBOUND_MESSAGE_QUEUE_SIZE: usize = 3072;
const DEFAULT_OUTBOUND_CONTROL_QUEUE_SIZE: usize = 20_000;
const DEFAULT_OUTBOUND_LARGE_MESSAGE_QUEUE_SIZE: usize = 256;
const DEFAULT_REMOTE_EVENT_QUEUE_SIZE: usize =
  DEFAULT_OUTBOUND_MESSAGE_QUEUE_SIZE + DEFAULT_OUTBOUND_CONTROL_QUEUE_SIZE;
const DEFAULT_OUTBOUND_HIGH_WATERMARK: usize = 1024;
const DEFAULT_OUTBOUND_LOW_WATERMARK: usize = 512;
const DEFAULT_REMOVE_QUARANTINED_ASSOCIATION_AFTER: Duration = Duration::from_secs(60 * 60);
const DEFAULT_COMPRESSION_ADVERTISEMENT_INTERVAL: Duration = Duration::from_secs(60);
const MINIMUM_MAXIMUM_FRAME_SIZE: usize = 32 * 1024;

fn non_zero(value: usize) -> NonZeroUsize {
  NonZeroUsize::new(value).expect("test value must be non-zero")
}

#[test]
fn new_uses_defaults_for_optional_fields() {
  let s = RemoteConfig::new("localhost");

  assert_eq!(s.canonical_host(), "localhost");
  assert_eq!(s.canonical_port(), None);
  assert_eq!(s.handshake_timeout(), Duration::from_secs(20));
  assert_eq!(s.deployment_timeout(), Duration::from_secs(60));
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
  assert_eq!(s.outbound_large_message_queue_size(), DEFAULT_OUTBOUND_LARGE_MESSAGE_QUEUE_SIZE);
  assert_eq!(s.remote_event_queue_size(), DEFAULT_REMOTE_EVENT_QUEUE_SIZE);
  assert_eq!(s.outbound_high_watermark(), DEFAULT_OUTBOUND_HIGH_WATERMARK);
  assert_eq!(s.outbound_low_watermark(), DEFAULT_OUTBOUND_LOW_WATERMARK);
  assert!(s.large_message_destinations().is_empty());
  assert_eq!(s.remove_quarantined_association_after(), DEFAULT_REMOVE_QUARANTINED_ASSOCIATION_AFTER);
  assert_eq!(s.compression_config().actor_ref_max(), Some(non_zero(256)));
  assert_eq!(s.compression_config().manifest_max(), Some(non_zero(256)));
  assert_eq!(s.compression_config().actor_ref_advertisement_interval(), DEFAULT_COMPRESSION_ADVERTISEMENT_INTERVAL);
  assert_eq!(s.compression_config().manifest_advertisement_interval(), DEFAULT_COMPRESSION_ADVERTISEMENT_INTERVAL);
  assert!(!s.untrusted_mode());
  assert!(!s.log_received_messages());
  assert!(!s.log_sent_messages());
  assert_eq!(s.log_frame_size_exceeding(), None);
}

#[test]
fn remote_compression_config_default_matches_new() {
  assert_eq!(RemoteCompressionConfig::default(), RemoteCompressionConfig::new());
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
    .with_deployment_timeout(Duration::from_secs(45))
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
  assert_eq!(s.deployment_timeout(), Duration::from_secs(45));
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
  let destinations = LargeMessageDestinations::new()
    .with_pattern(LargeMessageDestinationPattern::new("/user/large"))
    .with_pattern(LargeMessageDestinationPattern::new("/temp/session-ask-actor*"));
  let compression = RemoteCompressionConfig::new()
    .with_actor_ref_max(Some(non_zero(32)))
    .with_actor_ref_advertisement_interval(Duration::from_secs(10))
    .with_manifest_max(None)
    .with_manifest_advertisement_interval(Duration::from_secs(20));
  let s = RemoteConfig::new("localhost")
    .with_bind_hostname("0.0.0.0")
    .with_bind_port(25520)
    .with_outbound_large_message_queue_size(16)
    .with_large_message_destinations(destinations.clone())
    .with_compression_config(compression)
    .with_inbound_lanes(8)
    .with_outbound_lanes(2)
    .with_maximum_frame_size(512 * 1024)
    .with_buffer_pool_size(64)
    .with_outbound_message_queue_size(32)
    .with_outbound_control_queue_size(8)
    .with_remote_event_queue_size(64)
    .with_outbound_watermarks(16, 64)
    .with_remove_quarantined_association_after(Duration::from_secs(30))
    .with_untrusted_mode(true)
    .with_log_received_messages(true)
    .with_log_sent_messages(true)
    .with_log_frame_size_exceeding(128 * 1024);

  // Then: 上書きした値を保持する
  assert_eq!(s.bind_hostname(), Some("0.0.0.0"));
  assert_eq!(s.bind_port(), Some(25520));
  assert_eq!(s.outbound_large_message_queue_size(), 16);
  assert_eq!(s.large_message_destinations(), &destinations);
  assert_eq!(s.compression_config(), &compression);
  assert_eq!(s.inbound_lanes(), 8);
  assert_eq!(s.outbound_lanes(), 2);
  assert_eq!(s.maximum_frame_size(), 512 * 1024);
  assert_eq!(s.buffer_pool_size(), 64);
  assert_eq!(s.outbound_message_queue_size(), 32);
  assert_eq!(s.outbound_control_queue_size(), 8);
  assert_eq!(s.remote_event_queue_size(), 64);
  assert_eq!(s.outbound_high_watermark(), 64);
  assert_eq!(s.outbound_low_watermark(), 16);
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
fn with_outbound_large_message_queue_size_rejects_zero() {
  // When: outbound large-message queue size に 0 を指定する
  let result = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_large_message_queue_size(0));

  // Then: 不正な queue size として拒否する
  assert!(result.is_err());
}

#[test]
fn with_remote_event_queue_size_rejects_zero() {
  // When: remote event queue size に 0 を指定する
  let result = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_remote_event_queue_size(0));

  // Then: 不正な queue size として拒否する
  assert!(result.is_err());
}

#[test]
fn outbound_watermark_setters_are_order_independent() {
  // Given: default より小さい watermark に high -> low の順で変更する
  let high_first = RemoteConfig::new("localhost").with_outbound_high_watermark(256).with_outbound_low_watermark(128);

  // Given: 同じ watermark に low -> high の順で変更する
  let low_first_small =
    RemoteConfig::new("localhost").with_outbound_low_watermark(128).with_outbound_high_watermark(256);

  // Given: default より大きい watermark に high -> low の順で変更する
  let high_first_large =
    RemoteConfig::new("localhost").with_outbound_high_watermark(4096).with_outbound_low_watermark(2048);

  // Given: 同じ watermark に low -> high の順で変更する
  let low_first_large =
    RemoteConfig::new("localhost").with_outbound_low_watermark(2048).with_outbound_high_watermark(4096);

  // Then: どちらの順序でも最終的な組み合わせを保持する
  assert_eq!(high_first.outbound_high_watermark(), 256);
  assert_eq!(high_first.outbound_low_watermark(), 128);
  assert_eq!(low_first_small.outbound_high_watermark(), 256);
  assert_eq!(low_first_small.outbound_low_watermark(), 128);
  assert_eq!(high_first_large.outbound_high_watermark(), 4096);
  assert_eq!(high_first_large.outbound_low_watermark(), 2048);
  assert_eq!(low_first_large.outbound_high_watermark(), 4096);
  assert_eq!(low_first_large.outbound_low_watermark(), 2048);
}

#[test]
fn outbound_watermark_single_setters_keep_pair_valid() {
  // When: high を現在の low 以下に下げる
  let lowered_high = RemoteConfig::new("localhost").with_outbound_high_watermark(256);

  // Then: low は high 未満に補正される
  assert_eq!(lowered_high.outbound_high_watermark(), 256);
  assert_eq!(lowered_high.outbound_low_watermark(), 255);

  // When: low を現在の high 以上に上げる
  let raised_low = RemoteConfig::new("localhost").with_outbound_low_watermark(1024);

  // Then: high は low より大きく補正される
  assert_eq!(raised_low.outbound_high_watermark(), 1025);
  assert_eq!(raised_low.outbound_low_watermark(), 1024);
}

#[test]
fn with_outbound_watermarks_rejects_invalid_pairs() {
  let equal_result = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_watermarks(512, 512));
  let inverted_result = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_watermarks(1024, 512));

  assert!(equal_result.is_err());
  assert!(inverted_result.is_err());
}

#[test]
fn outbound_watermark_setters_reject_unreachable_values() {
  let high_zero = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_high_watermark(0));
  // high=1 だと auto-adjust で low=0 になり、release 条件 `queue_len < 0` が unreachable になるため
  // reject。
  let high_one = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_high_watermark(1));
  // low=0 は release 条件が unreachable なので reject。
  let low_zero = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_low_watermark(0));
  let low_max = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_low_watermark(usize::MAX));
  let pair_low_zero = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_watermarks(0, 1));

  assert!(high_zero.is_err());
  assert!(high_one.is_err());
  assert!(low_zero.is_err());
  assert!(low_max.is_err());
  assert!(pair_low_zero.is_err());
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
fn restart_timing_settings_reject_zero_duration() {
  let outbound_backoff =
    std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_restart_backoff(Duration::ZERO));
  let outbound_timeout =
    std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_outbound_restart_timeout(Duration::ZERO));

  assert!(outbound_backoff.is_err());
  assert!(outbound_timeout.is_err());
}

#[test]
fn with_deployment_timeout_rejects_zero() {
  let result = std::panic::catch_unwind(|| RemoteConfig::new("localhost").with_deployment_timeout(Duration::ZERO));

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
fn cloning_preserves_large_message_and_compression_immutability_of_original() {
  let a = RemoteConfig::new("localhost");
  let compression = RemoteCompressionConfig::new().with_manifest_max(None);
  let destinations = LargeMessageDestinations::new().with_pattern(LargeMessageDestinationPattern::new("/user/large"));
  let b = a
    .clone()
    .with_outbound_large_message_queue_size(8)
    .with_large_message_destinations(destinations.clone())
    .with_compression_config(compression);

  assert_eq!(a.outbound_large_message_queue_size(), DEFAULT_OUTBOUND_LARGE_MESSAGE_QUEUE_SIZE);
  assert!(a.large_message_destinations().is_empty());
  assert_ne!(a.compression_config(), &compression);
  assert_eq!(b.outbound_large_message_queue_size(), 8);
  assert_eq!(b.large_message_destinations(), &destinations);
  assert_eq!(b.compression_config(), &compression);
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

#[test]
fn large_message_destinations_match_exact_and_wildcard_paths() {
  let destinations = LargeMessageDestinations::new()
    .with_pattern(LargeMessageDestinationPattern::new("/user/largeMessageActor"))
    .with_pattern(LargeMessageDestinationPattern::new("/user/largeMessagesGroup/*"))
    .with_pattern(LargeMessageDestinationPattern::new("/user/thirdGroup/**"))
    .with_pattern(LargeMessageDestinationPattern::new("/temp/session-ask-actor*"));

  assert!(destinations.matches_absolute_path("/user/largeMessageActor"));
  assert!(destinations.matches_absolute_path("/user/largeMessagesGroup/actor1"));
  assert!(destinations.matches_absolute_path("/user/thirdGroup/actor3"));
  assert!(destinations.matches_absolute_path("/user/thirdGroup/actor4/actor5"));
  assert!(destinations.matches_absolute_path("/temp/session-ask-actor$abc"));
  assert!(!destinations.matches_absolute_path("/user/small"));
}

#[test]
fn large_message_destination_pattern_rejects_relative_path_without_leading_slash() {
  let result = std::panic::catch_unwind(|| LargeMessageDestinationPattern::new("user/large"));

  assert!(result.is_err());
}

#[test]
fn remote_compression_config_rejects_zero_advertisement_interval() {
  let actor_ref_result =
    std::panic::catch_unwind(|| RemoteCompressionConfig::new().with_actor_ref_advertisement_interval(Duration::ZERO));
  let manifest_result =
    std::panic::catch_unwind(|| RemoteCompressionConfig::new().with_manifest_advertisement_interval(Duration::ZERO));

  assert!(actor_ref_result.is_err());
  assert!(manifest_result.is_err());
}

#[test]
fn advanced_settings_sources_keep_no_std_boundary() {
  let sources = [
    include_str!("config/remote_config.rs"),
    include_str!("config/large_message_destination_pattern.rs"),
    include_str!("config/large_message_destinations.rs"),
    include_str!("config/remote_compression_config.rs"),
  ];

  for source in sources {
    assert!(!source.contains("use std::"), "remote-core config advanced settings must remain no_std");
  }
}
