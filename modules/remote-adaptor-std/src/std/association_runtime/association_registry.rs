//! Registry of per-remote `AssociationShared` handles.

use std::collections::BTreeMap;

use fraktor_remote_core_rs::core::address::UniqueAddress;

use crate::std::association_runtime::association_shared::AssociationShared;

/// Registry mapping a [`UniqueAddress`] to its [`AssociationShared`] handle.
///
/// Each remote node is represented by a single `AssociationShared`. The
/// registry itself is owned by the higher-level runtime (typically by
/// `StdRemoting` from Section 22) and is **not** internally synchronised —
/// callers either own it via `&mut self` or wrap the entire registry in an
/// outer lock if it must be shared across tasks.
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

  /// Returns a reference to the entry for `address` if present.
  #[must_use]
  pub fn get(&self, address: &UniqueAddress) -> Option<&AssociationShared> {
    self.entries.get(address)
  }

  /// Iterates over every `(address, shared)` pair in the registry.
  pub fn iter(&self) -> impl Iterator<Item = (&UniqueAddress, &AssociationShared)> {
    self.entries.iter()
  }
}
