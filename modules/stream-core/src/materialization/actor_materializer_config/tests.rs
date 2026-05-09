use core::time::Duration;

use crate::{
  SupervisionStrategy,
  materialization::{ActorMaterializerConfig, SubscriptionTimeoutConfig, SubscriptionTimeoutMode},
  stream_ref::StreamRefSettings,
};

// --- default values ---

#[test]
fn new_returns_default_supervision_strategy_stop() {
  let config = ActorMaterializerConfig::new();
  assert_eq!(config.supervision_strategy(), SupervisionStrategy::Stop);
}

#[test]
fn new_returns_default_subscription_timeout() {
  let config = ActorMaterializerConfig::new();
  let timeout = config.subscription_timeout();
  assert_eq!(timeout.mode(), SubscriptionTimeoutMode::Cancel);
  assert_eq!(timeout.timeout_ticks(), 5000);
}

#[test]
fn new_returns_default_debug_logging_false() {
  let config = ActorMaterializerConfig::new();
  assert!(!config.debug_logging());
}

#[test]
fn new_returns_default_output_burst_limit() {
  let config = ActorMaterializerConfig::new();
  assert_eq!(config.output_burst_limit(), 1000);
}

#[test]
fn new_returns_default_max_fixed_buffer_size() {
  let config = ActorMaterializerConfig::new();
  assert_eq!(config.max_fixed_buffer_size(), 1_000_000_000);
}

// --- with_* builder round-trip ---

#[test]
fn with_supervision_strategy_round_trip() {
  let config = ActorMaterializerConfig::new().with_supervision_strategy(SupervisionStrategy::Resume);
  assert_eq!(config.supervision_strategy(), SupervisionStrategy::Resume);
}

#[test]
fn with_subscription_timeout_round_trip() {
  let timeout = SubscriptionTimeoutConfig::new(SubscriptionTimeoutMode::Warn, 100);
  let config = ActorMaterializerConfig::new().with_subscription_timeout(timeout);
  assert_eq!(config.subscription_timeout().mode(), SubscriptionTimeoutMode::Warn);
  assert_eq!(config.subscription_timeout().timeout_ticks(), 100);
}

#[test]
fn with_debug_logging_round_trip() {
  let config = ActorMaterializerConfig::new().with_debug_logging(true);
  assert!(config.debug_logging());
}

#[test]
fn with_output_burst_limit_round_trip() {
  let config = ActorMaterializerConfig::new().with_output_burst_limit(42);
  assert_eq!(config.output_burst_limit(), 42);
}

#[test]
fn with_max_fixed_buffer_size_round_trip() {
  let config = ActorMaterializerConfig::new().with_max_fixed_buffer_size(8192);
  assert_eq!(config.max_fixed_buffer_size(), 8192);
}

// --- builder chaining preserves other fields ---

#[test]
fn builder_chaining_preserves_existing_fields() {
  let config = ActorMaterializerConfig::new()
    .with_drive_interval(Duration::from_millis(50))
    .with_supervision_strategy(SupervisionStrategy::Restart)
    .with_debug_logging(true)
    .with_output_burst_limit(500);

  assert_eq!(config.drive_interval(), Duration::from_millis(50));
  assert_eq!(config.supervision_strategy(), SupervisionStrategy::Restart);
  assert!(config.debug_logging());
  assert_eq!(config.output_burst_limit(), 500);
  // unchanged fields keep defaults
  assert_eq!(config.max_fixed_buffer_size(), 1_000_000_000);
  assert_eq!(config.subscription_timeout().mode(), SubscriptionTimeoutMode::Cancel);
}

// --- Default trait ---

#[test]
fn default_matches_new() {
  let from_new = ActorMaterializerConfig::new();
  let from_default = ActorMaterializerConfig::default();
  assert_eq!(from_new.drive_interval(), from_default.drive_interval());
  assert_eq!(from_new.supervision_strategy(), from_default.supervision_strategy());
  assert_eq!(from_new.debug_logging(), from_default.debug_logging());
  assert_eq!(from_new.output_burst_limit(), from_default.output_burst_limit());
  assert_eq!(from_new.max_fixed_buffer_size(), from_default.max_fixed_buffer_size());
}

// --- StreamRefSettings 連携 ---

#[test]
fn new_returns_default_stream_ref_settings() {
  // Given/When: materializer config を default で構築する
  let config = ActorMaterializerConfig::new();

  // Then: StreamRefSettings の reference.conf 相当 default が含まれる
  assert_eq!(config.stream_ref_settings(), StreamRefSettings::new());
}

#[test]
fn with_stream_ref_settings_round_trip() {
  // Given: 明示的な StreamRefSettings
  let stream_ref_settings = StreamRefSettings::new()
    .with_buffer_capacity(64)
    .with_demand_redelivery_interval_ticks(2)
    .with_subscription_timeout_ticks(45)
    .with_termination_received_before_completion_leeway_ticks(5);

  // When: ActorMaterializerConfig に設定する
  let config = ActorMaterializerConfig::new().with_stream_ref_settings(stream_ref_settings.clone());

  // Then: 同じ設定値が取得できる
  assert_eq!(config.stream_ref_settings(), stream_ref_settings);
}

#[test]
fn with_stream_ref_settings_preserves_existing_materializer_fields() {
  // Given: 既存 materializer fields を設定済みにする
  let stream_ref_settings = StreamRefSettings::new().with_buffer_capacity(64);
  let config = ActorMaterializerConfig::new()
    .with_drive_interval(Duration::from_millis(50))
    .with_debug_logging(true)
    .with_output_burst_limit(500)
    .with_stream_ref_settings(stream_ref_settings.clone());

  // Then: StreamRef settings 追加後も既存 fields は保持される
  assert_eq!(config.drive_interval(), Duration::from_millis(50));
  assert!(config.debug_logging());
  assert_eq!(config.output_burst_limit(), 500);
  assert_eq!(config.stream_ref_settings(), stream_ref_settings);
}

// --- SubscriptionTimeoutConfig のテスト ---

#[test]
fn subscription_timeout_config_new_stores_fields() {
  let config = SubscriptionTimeoutConfig::new(SubscriptionTimeoutMode::Noop, 42);
  assert_eq!(config.mode(), SubscriptionTimeoutMode::Noop);
  assert_eq!(config.timeout_ticks(), 42);
}

#[test]
fn subscription_timeout_config_default_is_cancel_5000() {
  let config = SubscriptionTimeoutConfig::default();
  assert_eq!(config.mode(), SubscriptionTimeoutMode::Cancel);
  assert_eq!(config.timeout_ticks(), 5000);
}

#[test]
fn subscription_timeout_mode_variants_are_distinct() {
  assert_ne!(SubscriptionTimeoutMode::Noop, SubscriptionTimeoutMode::Warn);
  assert_ne!(SubscriptionTimeoutMode::Warn, SubscriptionTimeoutMode::Cancel);
  assert_ne!(SubscriptionTimeoutMode::Noop, SubscriptionTimeoutMode::Cancel);
}

#[test]
fn subscription_timeout_config_is_copy() {
  let config = SubscriptionTimeoutConfig::new(SubscriptionTimeoutMode::Warn, 10);
  let copied = config;
  assert_eq!(config.mode(), copied.mode());
  assert_eq!(config.timeout_ticks(), copied.timeout_ticks());
}
