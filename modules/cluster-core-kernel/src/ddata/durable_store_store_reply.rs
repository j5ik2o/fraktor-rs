//! Optional reply contract for a durable store write.

/// Optional reply contract for a durable store write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableStoreStoreReply {
  store_succeeded: bool,
  store_failed:    bool,
}

impl DurableStoreStoreReply {
  /// Creates a reply contract with explicit success and failure markers.
  #[must_use]
  pub const fn new(store_succeeded: bool, store_failed: bool) -> Self {
    Self { store_succeeded, store_failed }
  }

  /// Returns the success marker carried to the requester on success.
  #[must_use]
  pub const fn store_succeeded(&self) -> bool {
    self.store_succeeded
  }

  /// Returns the failure marker carried to the requester on failure.
  #[must_use]
  pub const fn store_failed(&self) -> bool {
    self.store_failed
  }
}
