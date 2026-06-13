//! Typed keys for distributed-data entries.

#[cfg(test)]
#[path = "key_test.rs"]
mod tests;

use alloc::string::String;
use core::{
  hash::{Hash, Hasher},
  marker::PhantomData,
};

use super::{Flag, GCounter, PNCounter, PNCounterMap};

/// Typed key that identifies a CRDT value by string id.
#[derive(Debug)]
pub struct Key<T> {
  id:      String,
  _marker: PhantomData<fn() -> T>,
}

impl<T> Key<T> {
  /// Creates a new typed key.
  #[must_use]
  pub fn new(id: impl Into<String>) -> Self {
    Self { id: id.into(), _marker: PhantomData }
  }

  /// Returns the key id.
  #[must_use]
  pub fn id(&self) -> &str {
    &self.id
  }
}

impl<T> Clone for Key<T> {
  fn clone(&self) -> Self {
    Self { id: self.id.clone(), _marker: PhantomData }
  }
}

impl<T> PartialEq for Key<T> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl<T> Eq for Key<T> {}

impl<T> Hash for Key<T> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.id.hash(state);
  }
}

/// Key for [`Flag`] values.
pub type FlagKey = Key<Flag>;

/// Key for [`GCounter`] values.
pub type GCounterKey = Key<GCounter>;

/// Key for [`PNCounter`] values.
pub type PNCounterKey = Key<PNCounter>;

/// Key for [`PNCounterMap`] values.
pub type PNCounterMapKey<K> = Key<PNCounterMap<K>>;
