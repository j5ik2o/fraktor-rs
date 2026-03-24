use core::time::Duration;

use crate::core::{
  SubscriptionTimeoutMode, SubscriptionTimeoutSettings, SupervisionStrategy,
  mat::ActorMaterializerConfig,
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
  let timeout = SubscriptionTimeoutSettings::new(SubscriptionTimeoutMode::Warn, 100);
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

// --- SubscriptionTimeoutSettings ---

#[test]
fn subscription_timeout_settings_new_stores_fields() {
  let settings = SubscriptionTimeoutSettings::new(SubscriptionTimeoutMode::Noop, 42);
  assert_eq!(settings.mode(), SubscriptionTimeoutMode::Noop);
  assert_eq!(settings.timeout_ticks(), 42);
}

#[test]
fn subscription_timeout_settings_default_is_cancel_5000() {
  let settings = SubscriptionTimeoutSettings::default();
  assert_eq!(settings.mode(), SubscriptionTimeoutMode::Cancel);
  assert_eq!(settings.timeout_ticks(), 5000);
}

#[test]
fn subscription_timeout_mode_variants_are_distinct() {
  assert_ne!(SubscriptionTimeoutMode::Noop, SubscriptionTimeoutMode::Warn);
  assert_ne!(SubscriptionTimeoutMode::Warn, SubscriptionTimeoutMode::Cancel);
  assert_ne!(SubscriptionTimeoutMode::Noop, SubscriptionTimeoutMode::Cancel);
}

#[test]
fn subscription_timeout_settings_is_copy() {
  let settings = SubscriptionTimeoutSettings::new(SubscriptionTimeoutMode::Warn, 10);
  let copied = settings;
  assert_eq!(settings.mode(), copied.mode());
  assert_eq!(settings.timeout_ticks(), copied.timeout_ticks());
}
