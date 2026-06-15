//! Last-writer-wins register CRDT.

#[cfg(test)]
#[path = "lww_register_test.rs"]
mod tests;

use fraktor_remote_core_rs::address::UniqueAddress;

use crate::ddata::{ReplicatedData, SelfUniqueAddress};

/// Last-writer-wins register CRDT using timestamp and node ordering.
///
/// Merge selects the value with the greatest timestamp. If timestamps are equal, the value written
/// by the lowest [`UniqueAddress`] wins, matching Pekko's deterministic tie-break contract. Values
/// are not used as tie-break input. Clocks must not produce different writes from the same node
/// with the same timestamp; [`Self::with_value_with_clock`] returns [`None`] for local same-writer
/// timestamp reuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LWWRegister<T> {
  updated_by: UniqueAddress,
  value:      T,
  timestamp:  i64,
}

impl<T> LWWRegister<T> {
  /// Creates a register with the default last-writer-wins clock.
  #[must_use]
  pub fn new(node: &SelfUniqueAddress, value: T, current_time_millis: i64) -> Self {
    Self::new_with_clock(node, value, |current_timestamp, _| {
      Self::default_clock(current_timestamp, current_time_millis)
    })
  }

  /// Creates a register by asking `clock` for the first timestamp.
  ///
  /// The clock must not create two different writes from the same node with the same timestamp.
  #[must_use]
  pub fn new_with_clock(node: &SelfUniqueAddress, value: T, clock: impl FnOnce(i64, &T) -> i64) -> Self {
    let timestamp = clock(0, &value);
    Self::new_at(node, value, timestamp)
  }

  /// Returns the next timestamp for the default last-write-wins clock.
  ///
  /// This mirrors Pekko's default clock while keeping the core crate free of direct wall-clock
  /// access: callers supply the current wall-clock millis.
  #[must_use]
  pub const fn default_clock(current_timestamp: i64, current_time_millis: i64) -> i64 {
    let next_timestamp = current_timestamp.saturating_add(1);
    if current_time_millis > next_timestamp { current_time_millis } else { next_timestamp }
  }

  /// Returns the next timestamp for first-write-wins semantics.
  ///
  /// The supplied wall-clock millis is negated and used as an additional descending timestamp
  /// candidate, so unobserved reverse-clock writes can be ordered by supplied time before falling
  /// back to node ordering.
  #[must_use]
  pub const fn reverse_clock(current_timestamp: i64, current_time_millis: i64) -> i64 {
    let previous_timestamp = current_timestamp.saturating_sub(1);
    let reversed_time = current_time_millis.saturating_neg();
    if reversed_time < previous_timestamp { reversed_time } else { previous_timestamp }
  }

  fn new_at(node: &SelfUniqueAddress, value: T, timestamp: i64) -> Self {
    Self { updated_by: node.unique_address().clone(), value, timestamp }
  }

  /// Returns the current register value.
  #[must_use]
  pub const fn value(&self) -> &T {
    &self.value
  }

  /// Returns the timestamp attached to the current value.
  #[must_use]
  pub const fn timestamp(&self) -> i64 {
    self.timestamp
  }

  /// Returns the node that wrote the current value.
  #[must_use]
  pub const fn updated_by(&self) -> &UniqueAddress {
    &self.updated_by
  }

  /// Returns a register with a replacement value using the default last-writer-wins clock.
  #[must_use]
  pub fn with_value(&self, node: &SelfUniqueAddress, value: T, current_time_millis: i64) -> Option<Self> {
    self.with_value_with_clock(node, value, |current_timestamp, _| {
      Self::default_clock(current_timestamp, current_time_millis)
    })
  }

  /// Returns a register with a replacement value whose timestamp is selected by `clock`.
  ///
  /// Returns [`None`] when the same writer would reuse the current timestamp.
  #[must_use]
  pub fn with_value_with_clock(
    &self,
    node: &SelfUniqueAddress,
    value: T,
    clock: impl FnOnce(i64, &T) -> i64,
  ) -> Option<Self> {
    let timestamp = clock(self.timestamp, &value);
    if node.unique_address() == &self.updated_by && timestamp == self.timestamp {
      None
    } else {
      Some(Self::new_at(node, value, timestamp))
    }
  }
}

impl<T> ReplicatedData for LWWRegister<T>
where
  T: Clone,
{
  fn merge(&self, other: &Self) -> Self {
    if other.timestamp > self.timestamp || (other.timestamp == self.timestamp && other.updated_by < self.updated_by) {
      other.clone()
    } else {
      self.clone()
    }
  }
}
