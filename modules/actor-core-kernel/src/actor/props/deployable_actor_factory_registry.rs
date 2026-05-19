use alloc::{boxed::Box, collections::BTreeMap, string::String};

use fraktor_utils_core_rs::sync::ArcShared;

#[cfg(test)]
#[path = "deployable_actor_factory_registry_test.rs"]
mod tests;

use crate::actor::{
  messaging::AnyMessage,
  props::{DeployableActorFactory, DeployableFactoryLookupError, Props},
};

/// Registry that resolves stable deployable factory ids on the target node.
#[derive(Clone, Default)]
pub struct DeployableActorFactoryRegistry {
  entries: BTreeMap<String, ArcShared<Box<dyn DeployableActorFactory>>>,
}

impl DeployableActorFactoryRegistry {
  /// Creates an empty deployable factory registry.
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// Registers or replaces a deployable factory under a stable id.
  pub fn register(&mut self, factory_id: impl Into<String>, factory: Box<dyn DeployableActorFactory>) {
    self.entries.insert(factory_id.into(), ArcShared::new(factory));
  }

  /// Returns true when the factory id is registered.
  #[must_use]
  pub fn contains(&self, factory_id: &str) -> bool {
    self.entries.contains_key(factory_id)
  }

  /// Resolves a factory id and builds local props for the provided deployment payload.
  ///
  /// # Errors
  ///
  /// Returns [`DeployableFactoryLookupError::UnknownFactoryId`] when no factory is registered for
  /// the id, or [`DeployableFactoryLookupError::FactoryRejected`] when the registered factory
  /// rejects the payload.
  pub fn props_for_payload(
    &self,
    factory_id: &str,
    payload: AnyMessage,
  ) -> Result<Props, DeployableFactoryLookupError> {
    let factory =
      self.entries.get(factory_id).ok_or_else(|| DeployableFactoryLookupError::UnknownFactoryId(factory_id.into()))?;
    factory.props_for_payload(payload).map_err(DeployableFactoryLookupError::FactoryRejected)
  }
}
