use std::borrow::Cow;

use fraktor_stream_core_rs::core::{
  StreamDslError, StreamError,
  attributes::{
    Attributes, StreamRefBufferCapacity, StreamRefDemandRedeliveryInterval, StreamRefFinalTerminationSignalDeadline,
    StreamRefSubscriptionTimeout,
  },
  materialization::ActorMaterializerConfig,
  stream_ref::StreamRefSettings,
};

#[test]
fn stream_ref_settings_are_reachable_from_public_core_api() {
  // Given/When: crate public API から StreamRefSettings を構築する
  let settings = StreamRefSettings::new();

  // Then: reference.conf 相当の default 値へ到達できる
  assert_eq!(settings.buffer_capacity(), 32);
  assert_eq!(settings.demand_redelivery_interval_ticks(), 1);
  assert_eq!(settings.subscription_timeout_ticks(), 30);
  assert_eq!(settings.final_termination_signal_deadline_ticks(), 2);
}

#[test]
fn stream_ref_attributes_are_reachable_and_typed() {
  // Given: crate public API の StreamRef attributes factory を合成する
  let attributes = Attributes::stream_ref_subscription_timeout(30)
    .and(Attributes::stream_ref_buffer_capacity(32).expect("positive capacity must be accepted"))
    .and(Attributes::stream_ref_demand_redelivery_interval(1))
    .and(Attributes::stream_ref_final_termination_signal_deadline(2));

  // Then: 各 attribute は具体型で取り出せる
  assert_eq!(attributes.get::<StreamRefSubscriptionTimeout>().unwrap().timeout_ticks, 30);
  assert_eq!(attributes.get::<StreamRefBufferCapacity>().unwrap().capacity, 32);
  assert_eq!(attributes.get::<StreamRefDemandRedeliveryInterval>().unwrap().timeout_ticks, 1);
  assert_eq!(attributes.get::<StreamRefFinalTerminationSignalDeadline>().unwrap().timeout_ticks, 2);
}

#[test]
fn stream_ref_buffer_capacity_public_factory_rejects_zero() {
  // Given/When: public factory に capacity=0 を渡す
  let error = Attributes::stream_ref_buffer_capacity(0).expect_err("zero capacity must be rejected");

  // Then: StreamDslError として fail-fast する
  assert_eq!(error, StreamDslError::InvalidArgument {
    name:   "capacity",
    value:  0,
    reason: "must be greater than zero",
  });
}

#[test]
fn actor_materializer_config_exposes_stream_ref_settings() {
  // Given: StreamRef settings を明示的に差し替える
  let stream_ref_settings = StreamRefSettings::new()
    .with_buffer_capacity(64)
    .with_demand_redelivery_interval_ticks(2)
    .with_subscription_timeout_ticks(45)
    .with_termination_received_before_completion_leeway_ticks(5);

  // When: ActorMaterializerConfig に設定する
  let config = ActorMaterializerConfig::new().with_stream_ref_settings(stream_ref_settings.clone());

  // Then: config 経由で同じ設定が取得できる
  assert_eq!(config.stream_ref_settings(), stream_ref_settings);
}

#[test]
fn stream_ref_error_variants_are_reachable_from_public_stream_error() {
  // Given/When: StreamRef 固有エラーを public StreamError から構築する
  let target = StreamError::StreamRefTargetNotInitialized;
  let timeout = StreamError::StreamRefSubscriptionTimeout { message: Cow::Borrowed("subscription timed out") };
  let terminated =
    StreamError::RemoteStreamRefActorTerminated { message: Cow::Borrowed("remote stream ref actor terminated") };
  let invalid_seq = StreamError::InvalidSequenceNumber {
    expected_seq_nr: 10,
    got_seq_nr:      9,
    message:         Cow::Borrowed("invalid sequence"),
  };
  let invalid_partner = StreamError::InvalidPartnerActor {
    expected_ref: Cow::Borrowed("pekko://sys/user/expected"),
    got_ref:      Cow::Borrowed("pekko://sys/user/got"),
    message:      Cow::Borrowed("invalid partner"),
  };

  // Then: 各 variant は distinct な意味単位として保持される
  assert!(matches!(target, StreamError::StreamRefTargetNotInitialized));
  assert!(matches!(timeout, StreamError::StreamRefSubscriptionTimeout { .. }));
  assert!(matches!(terminated, StreamError::RemoteStreamRefActorTerminated { .. }));
  assert!(matches!(invalid_seq, StreamError::InvalidSequenceNumber { .. }));
  assert!(matches!(invalid_partner, StreamError::InvalidPartnerActor { .. }));
}
