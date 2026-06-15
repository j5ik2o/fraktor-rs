//! Last-writer-wins register CRDT.

#[cfg(test)]
#[path = "lww_register_test.rs"]
mod tests;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{ReplicatedData, SelfUniqueAddress};

/// Last-writer-wins register CRDT using timestamp and node ordering.
///
/// Merge selects the value with the greatest timestamp. If timestamps are equal, the value written
/// by the lowest [`UniqueAddress`] wins, matching Pekko's deterministic tie-break contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LWWRegister<T> {
  updated_by: UniqueAddress,
  value:      T,
  timestamp:  u64,
}

impl<T> LWWRegister<T> {
  /// Creates a register with an explicit timestamp.
  #[must_use]
  pub fn new(node: &SelfUniqueAddress, value: T, timestamp: u64) -> Self {
    Self { updated_by: node.unique_address().clone(), value, timestamp }
  }

  /// Creates a register by asking `clock` for the first timestamp.
  #[must_use]
  pub fn new_with_clock(node: &SelfUniqueAddress, value: T, clock: impl FnOnce(u64, &T) -> u64) -> Self {
    let timestamp = clock(0, &value);
    Self::new(node, value, timestamp)
  }

  /// Returns the current register value.
  #[must_use]
  pub const fn value(&self) -> &T {
    &self.value
  }

  /// Returns the timestamp attached to the current value.
  #[must_use]
  pub const fn timestamp(&self) -> u64 {
    self.timestamp
  }

  /// Returns the node that wrote the current value.
  #[must_use]
  pub const fn updated_by(&self) -> &UniqueAddress {
    &self.updated_by
  }

  /// Returns a register with a replacement value and explicit timestamp.
  #[must_use]
  pub fn with_value(&self, node: &SelfUniqueAddress, value: T, timestamp: u64) -> Self {
    self.with_value_with_clock(node, value, |_, _| timestamp)
  }

  /// Returns a register with a replacement value whose timestamp is selected by `clock`.
  #[must_use]
  pub fn with_value_with_clock(&self, node: &SelfUniqueAddress, value: T, clock: impl FnOnce(u64, &T) -> u64) -> Self {
    let timestamp = clock(self.timestamp, &value);
    Self::new(node, value, timestamp)
  }
}

impl<T> ReplicatedData for LWWRegister<T>
where
  T: Clone,
{
  fn merge(&self, other: &Self) -> Self {
    if other.timestamp > self.timestamp {
      other.clone()
    } else if other.timestamp < self.timestamp {
      self.clone()
    } else if other.updated_by < self.updated_by {
      other.clone()
    } else {
      self.clone()
    }
  }
}
