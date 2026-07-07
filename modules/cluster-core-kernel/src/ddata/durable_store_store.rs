//! Durable store store request protocol message.

use alloc::string::String;

use crate::ddata::{DurableDataEnvelope, ReplicatedData};

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

/// Request to persist one distributed-data entry.
///
/// This mirrors Pekko's `DurableStore.Store` message at the port level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableStoreStore<D: ReplicatedData> {
  key:   String,
  data:  DurableDataEnvelope<D>,
  reply: Option<DurableStoreStoreReply>,
}

impl<D: ReplicatedData> DurableStoreStore<D> {
  /// Creates a store request without an explicit reply contract.
  #[must_use]
  pub fn new(key: impl Into<String>, data: DurableDataEnvelope<D>) -> Self {
    Self { key: key.into(), data, reply: None }
  }

  /// Returns a store request with an explicit reply contract.
  #[must_use]
  pub fn with_reply(mut self, reply: DurableStoreStoreReply) -> Self {
    self.reply = Some(reply);
    self
  }

  /// Returns the durable key id.
  #[must_use]
  pub fn key(&self) -> &str {
    &self.key
  }

  /// Returns the data envelope to persist.
  #[must_use]
  pub fn data(&self) -> &DurableDataEnvelope<D> {
    &self.data
  }

  /// Returns the optional reply contract.
  #[must_use]
  pub const fn reply(&self) -> Option<&DurableStoreStoreReply> {
    self.reply.as_ref()
  }
}
