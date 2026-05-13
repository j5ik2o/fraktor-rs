//! Registry for durable state store providers.

#[cfg(test)]
#[path = "durable_state_store_registry_test.rs"]
mod tests;

use alloc::{
  boxed::Box,
  collections::{BTreeMap, btree_map::Entry},
  string::String,
};

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  durable_state_error::DurableStateError, durable_state_store::DurableStateStore,
  durable_state_store_provider::DurableStateStoreProvider,
};

/// Registry that resolves durable state store providers by identifier.
pub struct DurableStateStoreRegistry<A> {
  providers: BTreeMap<String, ArcShared<dyn DurableStateStoreProvider<A>>>,
}

impl<A: Send + 'static> DurableStateStoreRegistry<A> {
  /// Creates an empty durable state store registry.
  #[must_use]
  pub const fn empty() -> Self {
    Self { providers: BTreeMap::new() }
  }

  /// Registers a provider for the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`DurableStateError::ProviderAlreadyRegistered`] when the identifier already
  /// exists.
  pub fn register(
    &mut self,
    provider_id: impl Into<String>,
    provider: ArcShared<dyn DurableStateStoreProvider<A>>,
  ) -> Result<(), DurableStateError> {
    let provider_id = provider_id.into();
    match self.providers.entry(provider_id) {
      | Entry::Occupied(entry) => Err(DurableStateError::provider_already_registered(entry.key().clone())),
      | Entry::Vacant(entry) => {
        entry.insert(provider);
        Ok(())
      },
    }
  }

  /// Resolves a durable state store from the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`DurableStateError::ProviderNotFound`] when the identifier does not exist.
  pub fn resolve(&self, provider_id: &str) -> Result<Box<dyn DurableStateStore<A>>, DurableStateError> {
    let provider = self.providers.get(provider_id).ok_or_else(|| DurableStateError::provider_not_found(provider_id))?;
    Ok(provider.durable_state_store())
  }
}
