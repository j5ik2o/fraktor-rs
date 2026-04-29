//! Registry of per-remote `AssociationShared` handles.

use std::collections::BTreeMap;

use fraktor_remote_core_rs::core::address::{Address, UniqueAddress};

use crate::std::association_runtime::association_shared::AssociationShared;

/// Registry mapping a [`UniqueAddress`] to its [`AssociationShared`] handle.
///
/// Each remote node is represented by a single `AssociationShared`. The
/// registry itself is owned by the runtime driver and is **not** internally
/// synchronised — callers either own it via `&mut self` or wrap the entire
/// registry in an outer lock if it must be shared across tasks.
#[derive(Debug, Default)]
pub struct AssociationRegistry {
  entries: BTreeMap<UniqueAddress, AssociationShared>,
}

impl AssociationRegistry {
  /// Creates a new, empty registry.
  #[must_use]
  pub const fn new() -> Self {
    Self { entries: BTreeMap::new() }
  }

  /// Returns the number of associations currently tracked.
  #[must_use]
  pub fn len(&self) -> usize {
    self.entries.len()
  }

  /// Returns `true` when the registry holds no associations.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// Inserts the given [`AssociationShared`] under `address`, replacing any
  /// existing entry.
  pub fn insert(&mut self, address: UniqueAddress, shared: AssociationShared) {
    self.entries.insert(address, shared);
  }

  /// Removes the entry for `address` if present.
  pub fn remove(&mut self, address: &UniqueAddress) -> Option<AssociationShared> {
    self.entries.remove(address)
  }

  /// Removes quarantined entries whose removal deadline is due.
  ///
  /// Returns the removed addresses in key order.
  pub fn remove_quarantined_due(&mut self, now_ms: u64) -> Vec<UniqueAddress> {
    let mut removed = Vec::new();
    self.entries.retain(|address, shared| {
      let should_remove = shared.with_write(|association| association.is_quarantine_removal_due(now_ms));
      if should_remove {
        removed.push(address.clone());
      }
      !should_remove
    });
    removed
  }

  /// Returns a reference to the entry for `address` if present.
  #[must_use]
  pub fn get(&self, address: &UniqueAddress) -> Option<&AssociationShared> {
    self.entries.get(address)
  }

  /// Returns the entry whose unique address wraps `address`, if present.
  #[must_use]
  pub fn get_by_remote_address(&self, address: &Address) -> Option<&AssociationShared> {
    self.entries.iter().find_map(|(remote, shared)| (remote.address() == address).then_some(shared))
  }

  /// Iterates over every `(address, shared)` pair in the registry.
  pub fn iter(&self) -> impl Iterator<Item = (&UniqueAddress, &AssociationShared)> {
    self.entries.iter()
  }
}
