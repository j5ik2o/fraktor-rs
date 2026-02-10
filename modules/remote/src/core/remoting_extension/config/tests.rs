use core::time::Duration;

use super::RemotingExtensionConfig;

#[test]
fn remoting_extension_config_default_handshake_timeout_is_three_seconds() {
  let config = RemotingExtensionConfig::default();
  assert_eq!(config.handshake_timeout(), Duration::from_secs(3));
}

#[test]
fn remoting_extension_config_with_handshake_timeout_overrides_timeout() {
  let config = RemotingExtensionConfig::default().with_handshake_timeout(Duration::from_millis(750));
  assert_eq!(config.handshake_timeout(), Duration::from_millis(750));
}

#[test]
#[should_panic(expected = "handshake timeout must be >= 1 millisecond")]
fn remoting_extension_config_with_handshake_timeout_rejects_zero_duration() {
  let _ = RemotingExtensionConfig::default().with_handshake_timeout(Duration::from_millis(0));
}
