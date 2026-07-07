//! Typed entity declaration for cluster sharding.

#[cfg(test)]
#[path = "entity_test.rs"]
mod tests;

use alloc::string::String;
use core::marker::PhantomData;

use crate::{EntityContext, GrainTypeKey};

/// User-defined entity id derivation rule for bare messages.
type ExtractEntityIdFn<M> = fn(&M) -> Option<String>;

/// Placeholder behavior factory invoked when entity activation wiring lands.
pub type CreateBehaviorFn<M> = fn(&EntityContext<M>);

fn default_extract_entity_id<M>(_message: &M) -> Option<String> {
  None
}

/// Declares a sharded entity type and its local behavior factory placeholder.
///
/// This is the fraktor equivalent of Pekko's `Entity[M, E]`. The behavior
/// factory is stored but not invoked by the typed facade yet; kind registration
/// and typed reference resolution delegate to the kernel cluster extension.
pub struct Entity<M> {
  type_key:          GrainTypeKey<M>,
  extract_entity_id: ExtractEntityIdFn<M>,
  create_behavior:   CreateBehaviorFn<M>,
  _message:          PhantomData<fn() -> M>,
}

impl<M> Entity<M> {
  /// Creates an entity declaration for the given type key and behavior factory placeholder.
  #[must_use]
  pub fn new(type_key: GrainTypeKey<M>, create_behavior: CreateBehaviorFn<M>) -> Self {
    Self { type_key, extract_entity_id: default_extract_entity_id, create_behavior, _message: PhantomData }
  }

  /// Overrides the default entity id extraction rule.
  #[must_use]
  pub const fn with_entity_id_extractor(mut self, extract_entity_id: ExtractEntityIdFn<M>) -> Self {
    self.extract_entity_id = extract_entity_id;
    self
  }

  /// Returns the declared grain type key.
  #[must_use]
  pub const fn type_key(&self) -> &GrainTypeKey<M> {
    &self.type_key
  }

  /// Returns the stored entity id extraction rule.
  #[must_use]
  pub const fn extract_entity_id(&self) -> ExtractEntityIdFn<M> {
    self.extract_entity_id
  }

  /// Returns the stored behavior factory placeholder.
  #[must_use]
  pub const fn create_behavior(&self) -> CreateBehaviorFn<M> {
    self.create_behavior
  }

  /// Consumes this entity declaration and returns the wrapped type key.
  #[must_use]
  pub fn into_type_key(self) -> GrainTypeKey<M> {
    self.type_key
  }
}
