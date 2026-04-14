use core::time::Duration;

use super::ProducerControllerConfig;

#[test]
fn default_settings_can_be_created() {
  let settings = ProducerControllerConfig::new();
  assert_eq!(settings.durable_queue_request_timeout(), Duration::from_secs(3));
  assert_eq!(settings.durable_queue_retry_attempts(), 10);
  assert_eq!(settings.durable_queue_resend_first_interval(), Duration::from_secs(1));
  assert_eq!(settings.chunk_large_messages_bytes(), 0);
}

#[test]
fn builder_methods_override_and_preserve_other_fields() {
  let settings = ProducerControllerConfig::new()
    .with_durable_queue_request_timeout(Duration::from_millis(25))
    .with_durable_queue_retry_attempts(4)
    .with_durable_queue_resend_first_interval(Duration::from_millis(7))
    .with_chunk_large_messages_bytes(512);

  assert_eq!(settings.durable_queue_request_timeout(), Duration::from_millis(25));
  assert_eq!(settings.durable_queue_retry_attempts(), 4);
  assert_eq!(settings.durable_queue_resend_first_interval(), Duration::from_millis(7));
  assert_eq!(settings.chunk_large_messages_bytes(), 512);
}
