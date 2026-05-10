use super::StreamRefSettings;

#[test]
fn new_uses_reference_conf_defaults_as_ticks() {
  // Given/When: StreamRef settings を default で構築する
  let settings = StreamRefSettings::new();

  // Then: Pekko reference.conf の stream-ref default を tick 値として保持する
  assert_eq!(settings.buffer_capacity(), 32);
  assert_eq!(settings.demand_redelivery_interval_ticks(), 1);
  assert_eq!(settings.subscription_timeout_ticks(), 30);
  assert_eq!(settings.final_termination_signal_deadline_ticks(), 2);
}

#[test]
fn default_matches_new() {
  // Given/When: Default と new で構築する
  let from_new = StreamRefSettings::new();
  let from_default = StreamRefSettings::default();

  // Then: 両者は同じ設定値を持つ
  assert_eq!(from_new, from_default);
}

#[test]
fn with_buffer_capacity_returns_updated_copy() {
  // Given: default settings
  let original = StreamRefSettings::new();

  // When: buffer capacity だけを更新する
  let updated = original.clone().with_buffer_capacity(64);

  // Then: 更新後だけが新しい値を持ち、元の値は変わらない
  assert_eq!(original.buffer_capacity(), 32);
  assert_eq!(updated.buffer_capacity(), 64);
  assert_eq!(updated.demand_redelivery_interval_ticks(), original.demand_redelivery_interval_ticks());
  assert_eq!(updated.subscription_timeout_ticks(), original.subscription_timeout_ticks());
  assert_eq!(updated.final_termination_signal_deadline_ticks(), original.final_termination_signal_deadline_ticks());
}

#[test]
#[should_panic(expected = "stream ref buffer capacity must be greater than zero")]
fn with_buffer_capacity_rejects_zero() {
  // Given/When/Then: buffer capacity は runtime handoff の上限なので 0 を許容しない
  let _settings = StreamRefSettings::new().with_buffer_capacity(0);
}

#[test]
fn with_demand_redelivery_interval_ticks_returns_updated_copy() {
  // Given: default settings
  let original = StreamRefSettings::new();

  // When: demand redelivery interval だけを更新する
  let updated = original.clone().with_demand_redelivery_interval_ticks(5);

  // Then: interval だけが更新される
  assert_eq!(updated.buffer_capacity(), original.buffer_capacity());
  assert_eq!(updated.demand_redelivery_interval_ticks(), 5);
  assert_eq!(updated.subscription_timeout_ticks(), original.subscription_timeout_ticks());
  assert_eq!(updated.final_termination_signal_deadline_ticks(), original.final_termination_signal_deadline_ticks());
}

#[test]
fn with_subscription_timeout_ticks_returns_updated_copy() {
  // Given: default settings
  let original = StreamRefSettings::new();

  // When: subscription timeout だけを更新する
  let updated = original.clone().with_subscription_timeout_ticks(60);

  // Then: timeout だけが更新される
  assert_eq!(updated.buffer_capacity(), original.buffer_capacity());
  assert_eq!(updated.demand_redelivery_interval_ticks(), original.demand_redelivery_interval_ticks());
  assert_eq!(updated.subscription_timeout_ticks(), 60);
  assert_eq!(updated.final_termination_signal_deadline_ticks(), original.final_termination_signal_deadline_ticks());
}

#[test]
fn with_termination_received_before_completion_leeway_ticks_updates_final_deadline() {
  // Given: default settings
  let original = StreamRefSettings::new();

  // When: Pekko の withTerminationReceivedBeforeCompletionLeeway 相当を更新する
  let updated = original.clone().with_termination_received_before_completion_leeway_ticks(9);

  // Then: final termination deadline だけが更新される
  assert_eq!(updated.buffer_capacity(), original.buffer_capacity());
  assert_eq!(updated.demand_redelivery_interval_ticks(), original.demand_redelivery_interval_ticks());
  assert_eq!(updated.subscription_timeout_ticks(), original.subscription_timeout_ticks());
  assert_eq!(updated.final_termination_signal_deadline_ticks(), 9);
}

#[test]
fn chained_with_methods_preserve_each_stream_ref_setting() {
  // Given/When: 4 種類の StreamRef setting を chain で更新する
  let settings = StreamRefSettings::new()
    .with_buffer_capacity(128)
    .with_demand_redelivery_interval_ticks(3)
    .with_subscription_timeout_ticks(45)
    .with_termination_received_before_completion_leeway_ticks(6);

  // Then: すべての値が独立して保持される
  assert_eq!(settings.buffer_capacity(), 128);
  assert_eq!(settings.demand_redelivery_interval_ticks(), 3);
  assert_eq!(settings.subscription_timeout_ticks(), 45);
  assert_eq!(settings.final_termination_signal_deadline_ticks(), 6);
}
