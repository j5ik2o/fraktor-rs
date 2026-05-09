//! Ack PDU: system message ack-based delivery.

/// Wire-level ack / nack for system message delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AckPdu {
  sequence_number: u64,
  cumulative_ack:  u64,
  nack_bitmap:     u64,
}

impl AckPdu {
  /// Creates a new [`AckPdu`].
  #[must_use]
  pub const fn new(sequence_number: u64, cumulative_ack: u64, nack_bitmap: u64) -> Self {
    Self { sequence_number, cumulative_ack, nack_bitmap }
  }

  /// Returns the sequence number of the latest acknowledged message.
  #[must_use]
  pub const fn sequence_number(&self) -> u64 {
    self.sequence_number
  }

  /// Returns the cumulative ack value.
  #[must_use]
  pub const fn cumulative_ack(&self) -> u64 {
    self.cumulative_ack
  }

  /// Returns the bitmap of nacked offsets (bit `i` set means offset
  /// `cumulative_ack + i + 1` is missing).
  #[must_use]
  pub const fn nack_bitmap(&self) -> u64 {
    self.nack_bitmap
  }
}
