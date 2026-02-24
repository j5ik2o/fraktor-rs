#![cfg(any(test, feature = "test-support"))]

use alloc::vec::Vec;

use super::RemoteInstrument;
use crate::core::wire_error::WireError;

struct ProbeInstrument;

impl RemoteInstrument for ProbeInstrument {
  fn identifier(&self) -> u8 {
    9
  }

  fn remote_write_metadata(&self, buffer: &mut Vec<u8>) {
    buffer.extend_from_slice(&[1, 2, 3]);
  }

  fn remote_message_sent(&self, _size: usize, _serialization_nanos: u64) {}

  fn remote_read_metadata(&self, buffer: &[u8]) -> Result<(), WireError> {
    if buffer == [1, 2, 3] { Ok(()) } else { Err(WireError::InvalidFormat) }
  }

  fn remote_message_received(&self, _size: usize, _deserialization_nanos: u64) {}
}

#[test]
fn default_serialization_timing_is_disabled() {
  let instrument = ProbeInstrument;
  assert_eq!(instrument.identifier(), 9);
  assert!(!instrument.serialization_timing_enabled());
}

#[test]
fn metadata_round_trip_works() {
  let instrument = ProbeInstrument;
  let mut metadata = Vec::new();
  instrument.remote_write_metadata(&mut metadata);
  assert_eq!(metadata, vec![1, 2, 3]);
  instrument.remote_read_metadata(&metadata).expect("metadata read");
}
