//! Registry managing activated cluster kinds.

#[cfg(test)]
mod tests;

use alloc::{collections::BTreeMap, string::String, vec::Vec};

use crate::core::activated_kind::ActivatedKind;

/// Default topic actor kind name, aligned with protoactor-go.
pub const TOPIC_ACTOR_KIND: &str = "prototopic";

/// Maintains the set of activated kinds, including the mandatory topic kind.
pub struct KindRegistry {
  kinds: BTreeMap<String, ActivatedKind>,
}

impl KindRegistry {
  /// Creates an empty registry.
  #[must_use]
  pub fn new() -> Self {
    Self { kinds: BTreeMap::new() }
  }

  /// Registers the provided kinds and ensures the topic actor kind is present.
  pub fn register_all(&mut self, kinds: Vec<ActivatedKind>) {
    for kind in kinds {
      self.kinds.insert(kind.name().to_string(), kind);
    }
    self.ensure_topic_actor_kind();
  }

  /// Returns all registered kinds as a vector.
  #[must_use]
  pub fn all(&self) -> Vec<ActivatedKind> {
    self.kinds.values().cloned().collect()
  }

  /// Returns true if a kind with the given name exists.
  #[must_use]
  pub fn contains(&self, name: &str) -> bool {
    self.kinds.contains_key(name)
  }

  fn ensure_topic_actor_kind(&mut self) {
    self.kinds
      .entry(TOPIC_ACTOR_KIND.to_string())
      .or_insert_with(|| ActivatedKind::new(TOPIC_ACTOR_KIND));
  }
}

impl Default for KindRegistry {
  fn default() -> Self {
    Self::new()
  }
}
