//! Represents an outbound transport channel.

/// Channel identifier allocated by the transport implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransportChannel {
  id: u64,
}

impl TransportChannel {
  /// Creates a new channel reference from the provided identifier.
  #[must_use]
  pub const fn new(id: u64) -> Self {
    Self { id }
  }

  /// Returns the unique identifier assigned to the channel.
  #[must_use]
  pub const fn id(&self) -> u64 {
    self.id
  }
}
