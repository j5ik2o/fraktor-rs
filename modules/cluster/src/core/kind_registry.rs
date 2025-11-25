//! Registry managing activated cluster kinds.

#[cfg(test)]
mod tests;

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec::Vec,
};

use crate::core::activated_kind::ActivatedKind;

/// Default topic actor kind name, aligned with protoactor-go.
pub const TOPIC_ACTOR_KIND: &str = "prototopic";

/// Maintains the set of activated kinds, including the mandatory topic kind.
pub struct KindRegistry {
  kinds:            BTreeMap<String, ActivatedKind>,
  invalid_requests: Vec<String>,
  generation:       u64,
  last_snapshot:    Vec<String>,
}

impl KindRegistry {
  /// Creates an empty registry.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      kinds:            BTreeMap::new(),
      invalid_requests: Vec::new(),
      generation:       0,
      last_snapshot:    Vec::new(),
    }
  }

  /// Registers the provided kinds and ensures the topic actor kind is present.
  pub fn register_all(&mut self, kinds: Vec<ActivatedKind>) {
    let before = self.last_snapshot.clone();
    for kind in kinds {
      self.kinds.insert(kind.name().to_string(), kind);
    }
    self.ensure_topic_actor_kind();
    let after = self.snapshot_names();
    if before != after {
      self.generation = self.generation.saturating_add(1);
      self.last_snapshot = after;
    }
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

  /// Retrieves a kind by name, recording invalid lookups.
  pub fn get(&mut self, name: &str) -> Option<ActivatedKind> {
    let result = self.kinds.get(name).cloned();
    if result.is_none() {
      self.invalid_requests.push(name.to_string());
    }
    result
  }

  /// Returns and clears the recorded invalid requests.
  #[must_use]
  pub fn take_invalid_requests(&mut self) -> Vec<String> {
    core::mem::take(&mut self.invalid_requests)
  }

  /// Returns the current generation (increments only when set changes).
  #[must_use]
  pub const fn generation(&self) -> u64 {
    self.generation
  }

  /// Aggregates virtual actor count from registered kinds.
  #[must_use]
  pub fn virtual_actor_count(&self) -> i64 {
    self.kinds.len() as i64
  }

  fn ensure_topic_actor_kind(&mut self) {
    self.kinds.entry(TOPIC_ACTOR_KIND.to_string()).or_insert_with(|| ActivatedKind::new(TOPIC_ACTOR_KIND));
  }

  fn snapshot_names(&self) -> Vec<String> {
    let mut names: Vec<_> = self.kinds.keys().cloned().collect();
    names.sort();
    names
  }
}

impl Default for KindRegistry {
  fn default() -> Self {
    Self::new()
  }
}
