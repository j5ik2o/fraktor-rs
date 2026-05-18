/// Normal StreamRef completion sent by a remote endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamRefRemoteStreamCompleted {
  seq_nr: u64,
}

impl StreamRefRemoteStreamCompleted {
  /// Creates a remote stream completion protocol payload.
  #[must_use]
  pub const fn new(seq_nr: u64) -> Self {
    Self { seq_nr }
  }

  /// Returns the completion sequence number.
  #[must_use]
  pub const fn seq_nr(&self) -> u64 {
    self.seq_nr
  }
}
