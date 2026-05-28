//! State-sourced effector configuration.
//!
//! Store recovery, upsert, and delete failures are fatal for the current
//! state-sourced effector behavior. This configuration intentionally does not
//! expose a retry or backoff policy until state-store retry semantics are
//! modeled explicitly.

#[cfg(test)]
#[path = "state_sourced_effector_config_test.rs"]
mod tests;

use alloc::string::String;

use fraktor_persistence_core_kernel_rs::{error::PersistenceError, state::DurableStateStoreProvider};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::{PersistenceId, StateSourcedEffectorMessageAdapter};

/// Configuration used to build a typed state-sourced effector.
pub struct StateSourcedEffectorConfig<S, M>
where
  S: Send + 'static, {
  persistence_id:  PersistenceId,
  stash_capacity:  usize,
  store_provider:  Option<ArcShared<dyn DurableStateStoreProvider<S>>>,
  message_adapter: Option<StateSourcedEffectorMessageAdapter<S, M>>,
}

impl<S, M> StateSourcedEffectorConfig<S, M>
where
  S: Send + 'static,
{
  /// Creates a configuration for a durable state persistence identifier.
  #[must_use]
  pub fn new(persistence_id: PersistenceId) -> Self {
    Self { persistence_id, stash_capacity: 1000, store_provider: None, message_adapter: None }
  }

  /// Returns the persistence id.
  #[must_use]
  pub const fn persistence_id(&self) -> &PersistenceId {
    &self.persistence_id
  }

  /// Returns the stash capacity.
  #[must_use]
  pub const fn stash_capacity(&self) -> usize {
    self.stash_capacity
  }

  /// Returns the optional message adapter.
  #[must_use]
  pub const fn message_adapter(&self) -> Option<&StateSourcedEffectorMessageAdapter<S, M>> {
    self.message_adapter.as_ref()
  }

  /// Returns the configured durable state store provider.
  #[must_use]
  pub fn store_provider(&self) -> Option<&ArcShared<dyn DurableStateStoreProvider<S>>> {
    self.store_provider.as_ref()
  }

  /// Returns a config with the selected stash capacity.
  #[must_use]
  pub const fn with_stash_capacity(mut self, stash_capacity: usize) -> Self {
    self.stash_capacity = stash_capacity;
    self
  }

  /// Returns a config with the selected message adapter.
  #[must_use]
  pub fn with_message_adapter(mut self, message_adapter: StateSourcedEffectorMessageAdapter<S, M>) -> Self {
    self.message_adapter = Some(message_adapter);
    self
  }

  /// Returns a config with the selected durable state store provider.
  #[must_use]
  pub fn with_store_provider(mut self, store_provider: ArcShared<dyn DurableStateStoreProvider<S>>) -> Self {
    self.store_provider = Some(store_provider);
    self
  }

  /// Validates the configuration.
  pub fn validate(&self) -> Result<(), PersistenceError> {
    if self.stash_capacity == 0 {
      return Err(validation_error("stash_capacity must be greater than 0"));
    }
    if self.message_adapter.is_none() {
      return Err(validation_error("state-sourced message adapter must be configured"));
    }
    if self.store_provider.is_none() {
      return Err(validation_error("durable state store provider must be configured"));
    }
    Ok(())
  }
}

impl<S, M> Clone for StateSourcedEffectorConfig<S, M>
where
  S: Send + 'static,
{
  fn clone(&self) -> Self {
    Self {
      persistence_id:  self.persistence_id.clone(),
      stash_capacity:  self.stash_capacity,
      store_provider:  self.store_provider.clone(),
      message_adapter: self.message_adapter.clone(),
    }
  }
}

fn validation_error(message: &str) -> PersistenceError {
  PersistenceError::StateMachine(String::from(message))
}
