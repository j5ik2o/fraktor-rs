//! Instrumentation contract for remote message metadata and timing hooks.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::core::wire_error::WireError;

/// Hook invoked by remoting encoders/decoders to observe wire metadata and timings.
pub trait RemoteInstrument: Send + Sync + 'static {
  /// Returns the instrument identifier used on the wire metadata section.
  fn identifier(&self) -> u8;

  /// Returns whether serialization/deserialization timings are required.
  #[must_use]
  fn serialization_timing_enabled(&self) -> bool {
    false
  }

  /// Writes outbound metadata bytes into the provided buffer.
  fn remote_write_metadata(&self, buffer: &mut Vec<u8>);

  /// Called after the message has been serialized and sent.
  fn remote_message_sent(&self, size: usize, serialization_nanos: u64);

  /// Reads inbound metadata bytes from the provided slice.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when metadata cannot be decoded.
  fn remote_read_metadata(&self, buffer: &[u8]) -> Result<(), WireError>;

  /// Called after the message has been decoded.
  fn remote_message_received(&self, size: usize, deserialization_nanos: u64);
}
