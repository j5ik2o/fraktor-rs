#![cfg(any(test, feature = "test-support"))]

use alloc::{sync::Arc, vec::Vec};
use std::sync::Mutex;

use super::RemoteInstruments;
use crate::core::{RemoteInstrument, WireError};

#[derive(Default)]
struct InstrumentProbeState {
  read_payloads:          Vec<Vec<u8>>,
  sent_notifications:     usize,
  received_notifications: usize,
}

struct ProbeInstrument {
  identifier: u8,
  metadata:   Vec<u8>,
  state:      Arc<Mutex<InstrumentProbeState>>,
}

impl ProbeInstrument {
  fn new(identifier: u8, metadata: Vec<u8>, state: Arc<Mutex<InstrumentProbeState>>) -> Self {
    Self { identifier, metadata, state }
  }
}

impl RemoteInstrument for ProbeInstrument {
  fn identifier(&self) -> u8 {
    self.identifier
  }

  fn remote_write_metadata(&self, buffer: &mut Vec<u8>) {
    buffer.extend_from_slice(&self.metadata);
  }

  fn remote_message_sent(&self, _size: usize, _serialization_nanos: u64) {
    let mut guard = self.state.lock().expect("state lock");
    guard.sent_notifications += 1;
  }

  fn remote_read_metadata(&self, buffer: &[u8]) -> Result<(), WireError> {
    let mut guard = self.state.lock().expect("state lock");
    guard.read_payloads.push(buffer.to_vec());
    Ok(())
  }

  fn remote_message_received(&self, _size: usize, _deserialization_nanos: u64) {
    let mut guard = self.state.lock().expect("state lock");
    guard.received_notifications += 1;
  }
}

#[test]
fn metadata_round_trip_dispatches_to_matching_instruments() {
  let first_state = Arc::new(Mutex::new(InstrumentProbeState::default()));
  let second_state = Arc::new(Mutex::new(InstrumentProbeState::default()));
  let instruments = RemoteInstruments::new(vec![
    Arc::new(ProbeInstrument::new(1, vec![0xAA], first_state.clone())),
    Arc::new(ProbeInstrument::new(2, vec![0xBB, 0xCC], second_state.clone())),
  ]);

  let payload = instruments.write_metadata();
  instruments.read_metadata(&payload).expect("metadata decode");

  assert_eq!(first_state.lock().expect("state lock").read_payloads, vec![vec![0xAA]]);
  assert_eq!(second_state.lock().expect("state lock").read_payloads, vec![vec![0xBB, 0xCC]]);
}

#[test]
fn metadata_decode_rejects_truncated_entry() {
  let instruments = RemoteInstruments::new(Vec::new());
  let payload = vec![7, 2, 0, 0, 0, 0x11];
  let error = instruments.read_metadata(&payload).expect_err("truncated metadata should fail");
  assert!(matches!(error, WireError::InvalidFormat));
}

#[test]
fn message_notifications_are_fanned_out_to_all_instruments() {
  let first_state = Arc::new(Mutex::new(InstrumentProbeState::default()));
  let second_state = Arc::new(Mutex::new(InstrumentProbeState::default()));
  let instruments = RemoteInstruments::new(vec![
    Arc::new(ProbeInstrument::new(11, Vec::new(), first_state.clone())),
    Arc::new(ProbeInstrument::new(12, Vec::new(), second_state.clone())),
  ]);

  instruments.message_sent(128, 77);
  instruments.message_received(256, 88);

  let first = first_state.lock().expect("state lock");
  assert_eq!(first.sent_notifications, 1);
  assert_eq!(first.received_notifications, 1);
  drop(first);
  let second = second_state.lock().expect("state lock");
  assert_eq!(second.sent_notifications, 1);
  assert_eq!(second.received_notifications, 1);
}
