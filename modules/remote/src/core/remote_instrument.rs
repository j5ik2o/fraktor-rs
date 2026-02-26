//! Instrumentation contract for remote message metadata and timing hooks.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::core::wire_error::WireError;

/// Hook invoked by remoting encoders/decoders to observe wire metadata and timings.
pub trait RemoteInstrument: Send + Sync + 'static {
  /// Returns the instrument identifier used on the wire metadata section.
  ///
  /// The value must be unique across all registered instruments. At most 256
  /// instruments (0..=255) can be registered because the identifier is `u8`.
  fn identifier(&self) -> u8;

  /// Returns whether serialization/deserialization timings are required.
  #[must_use]
  fn serialization_timing_enabled(&self) -> bool {
    false
  }

  /// Writes outbound metadata bytes into the provided buffer.
  ///
  /// `buffer` is the metadata section that will be appended to the wire frame.
  fn remote_write_metadata(&self, buffer: &mut Vec<u8>);

  /// Called after the message has been serialized and sent.
  ///
  /// `size` is the total wire frame size in bytes.
  /// `serialization_nanos` is the elapsed time in nanoseconds spent serializing.
  fn remote_message_sent(&self, size: usize, serialization_nanos: u64);

  /// Reads inbound metadata bytes from the provided slice.
  ///
  /// `buffer` contains the metadata section extracted from the wire frame.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when metadata cannot be decoded.
  fn remote_read_metadata(&self, buffer: &[u8]) -> Result<(), WireError>;

  /// Called after the message has been decoded.
  ///
  /// `size` is the total wire frame size in bytes.
  /// `deserialization_nanos` is the elapsed time in nanoseconds spent deserializing.
  fn remote_message_received(&self, size: usize, deserialization_nanos: u64);
}
