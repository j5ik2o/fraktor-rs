use fraktor_actor_core_kernel_rs::serialization::SerializedMessage;

/// Sequenced element payload sent by a StreamRef producer endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRefSequencedOnNext {
  seq_nr:  u64,
  payload: SerializedMessage,
}

impl StreamRefSequencedOnNext {
  /// Creates a sequenced element protocol payload.
  #[must_use]
  pub const fn new(seq_nr: u64, payload: SerializedMessage) -> Self {
    Self { seq_nr, payload }
  }

  /// Returns the element sequence number.
  #[must_use]
  pub const fn seq_nr(&self) -> u64 {
    self.seq_nr
  }

  /// Returns the serialized element payload.
  #[must_use]
  pub const fn payload(&self) -> &SerializedMessage {
    &self.payload
  }
}
