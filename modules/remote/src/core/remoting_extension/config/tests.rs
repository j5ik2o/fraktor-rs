use alloc::{sync::Arc, vec::Vec};
use core::time::Duration;

use super::RemotingExtensionConfig;
use crate::core::{RemoteInstrument, WireError};

struct NoopInstrument;

impl RemoteInstrument for NoopInstrument {
  fn identifier(&self) -> u8 {
    1
  }

  fn remote_write_metadata(&self, _buffer: &mut Vec<u8>) {}

  fn remote_message_sent(&self, _size: usize, _serialization_nanos: u64) {}

  fn remote_read_metadata(&self, _buffer: &[u8]) -> Result<(), WireError> {
    Ok(())
  }

  fn remote_message_received(&self, _size: usize, _deserialization_nanos: u64) {}
}

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

#[test]
fn remoting_extension_config_registers_remote_instrument() {
  let config = RemotingExtensionConfig::default().with_remote_instrument(Arc::new(NoopInstrument));
  assert_eq!(config.remote_instruments().len(), 1);
}

#[test]
fn remoting_extension_config_default_ack_windows_are_set() {
  let config = RemotingExtensionConfig::default();
  assert_eq!(config.ack_send_window(), 128);
  assert_eq!(config.ack_receive_window(), 128);
}

#[test]
fn remoting_extension_config_with_ack_send_window_overrides_value() {
  let config = RemotingExtensionConfig::default().with_ack_send_window(32);
  assert_eq!(config.ack_send_window(), 32);
}

#[test]
fn remoting_extension_config_with_ack_receive_window_overrides_value() {
  let config = RemotingExtensionConfig::default().with_ack_receive_window(64);
  assert_eq!(config.ack_receive_window(), 64);
}

#[test]
#[should_panic(expected = "ack send window must be > 0")]
fn remoting_extension_config_with_ack_send_window_rejects_zero() {
  let _ = RemotingExtensionConfig::default().with_ack_send_window(0);
}

#[test]
#[should_panic(expected = "ack receive window must be > 0")]
fn remoting_extension_config_with_ack_receive_window_rejects_zero() {
  let _ = RemotingExtensionConfig::default().with_ack_receive_window(0);
}
