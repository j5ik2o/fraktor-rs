//! Aggregates remote instruments and applies metadata/timing hooks.

#[cfg(test)]
mod tests;

use alloc::{sync::Arc, vec::Vec};
use core::convert::TryInto;

use crate::core::{remote_instrument::RemoteInstrument, wire_error::WireError};

/// Collection of remoting instruments used by transport pipelines.
pub(crate) struct RemoteInstruments {
  instruments:                  Vec<Arc<dyn RemoteInstrument>>,
  serialization_timing_enabled: bool,
}

impl RemoteInstruments {
  /// Creates a collection from the provided instrument instances.
  #[must_use]
  pub(crate) fn new(instruments: Vec<Arc<dyn RemoteInstrument>>) -> Self {
    assert!(
      {
        let mut ids: Vec<u8> = instruments.iter().map(|i| i.identifier()).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        ids.len() == before
      },
      "duplicate instrument identifiers detected"
    );
    let serialization_timing_enabled = instruments.iter().any(|instrument| instrument.serialization_timing_enabled());
    Self { instruments, serialization_timing_enabled }
  }

  /// Returns whether any instrument requests serialization/deserialization timings.
  #[must_use]
  pub(crate) const fn serialization_timing_enabled(&self) -> bool {
    self.serialization_timing_enabled
  }

  /// Serializes metadata entries produced by registered instruments.
  #[must_use]
  pub(crate) fn write_metadata(&self) -> Vec<u8> {
    let mut payload = Vec::new();
    for instrument in &self.instruments {
      let mut metadata = Vec::new();
      instrument.remote_write_metadata(&mut metadata);
      if metadata.is_empty() {
        continue;
      }
      payload.push(instrument.identifier());
      let metadata_len = u32::try_from(metadata.len()).expect("instrument metadata must fit in u32");
      payload.extend_from_slice(&metadata_len.to_le_bytes());
      payload.extend_from_slice(&metadata);
    }
    payload
  }

  /// Deserializes metadata entries and dispatches them to registered instruments.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when metadata framing is malformed or an instrument rejects its entry.
  pub(crate) fn read_metadata(&self, payload: &[u8]) -> Result<(), WireError> {
    let mut cursor = 0usize;
    while cursor < payload.len() {
      let header_end = cursor.checked_add(5).ok_or(WireError::InvalidFormat)?;
      if payload.len() < header_end {
        return Err(WireError::InvalidFormat);
      }
      let identifier = payload[cursor];
      cursor += 1;
      let metadata_len =
        u32::from_le_bytes(payload[cursor..cursor + 4].try_into().map_err(|_| WireError::InvalidFormat)?) as usize;
      cursor += 4;
      let metadata_end = cursor.checked_add(metadata_len).ok_or(WireError::InvalidFormat)?;
      if payload.len() < metadata_end {
        return Err(WireError::InvalidFormat);
      }
      let metadata = &payload[cursor..metadata_end];
      cursor = metadata_end;
      if let Some(instrument) = self.instruments.iter().find(|instrument| instrument.identifier() == identifier) {
        instrument.remote_read_metadata(metadata)?;
      }
    }
    Ok(())
  }

  /// Notifies instruments that an outbound message has been emitted.
  pub(crate) fn message_sent(&self, size: usize, serialization_nanos: u64) {
    for instrument in &self.instruments {
      instrument.remote_message_sent(size, serialization_nanos);
    }
  }

  /// Notifies instruments that an inbound message has been decoded.
  pub(crate) fn message_received(&self, size: usize, deserialization_nanos: u64) {
    for instrument in &self.instruments {
      instrument.remote_message_received(size, deserialization_nanos);
    }
  }
}

impl Default for RemoteInstruments {
  fn default() -> Self {
    Self::new(Vec::new())
  }
}
