use alloc::{borrow::ToOwned, string::String};

use ahash::RandomState;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::sync_mutex_like::SyncMutexLike,
};
use hashbrown::HashMap;

use crate::core::{config::ConfigError, props::DispatcherConfigGeneric};

#[cfg(test)]
mod tests;

const DEFAULT_DISPATCHER_ID: &str = "default";

/// Registry that resolves dispatcher identifiers to configurations.
pub struct DispatchersGeneric<TB: RuntimeToolbox + 'static> {
  entries: ToolboxMutex<HashMap<String, DispatcherConfigGeneric<TB>, RandomState>, TB>,
}

/// Type alias using the default toolbox.
pub type Dispatchers = DispatchersGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> DispatchersGeneric<TB> {
  /// Creates an empty dispatcher registry.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::with_hasher(RandomState::new())) }
  }

  /// Registers a dispatcher configuration for the provided identifier.
  ///
  /// # Errors
  ///
  /// Returns [`ConfigError::DispatcherDuplicate`] when the identifier already exists.
  pub fn register(&self, id: impl Into<String>, config: DispatcherConfigGeneric<TB>) -> Result<(), ConfigError> {
    let mut entries = self.entries.lock();
    let id = id.into();
    if entries.contains_key(&id) {
      return Err(ConfigError::dispatcher_duplicate(&id));
    }
    entries.insert(id, config);
    Ok(())
  }

  /// Resolves the dispatcher configuration for the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`ConfigError::DispatcherUnknown`] when the identifier has not been registered.
  pub fn resolve(&self, id: &str) -> Result<DispatcherConfigGeneric<TB>, ConfigError> {
    self.entries.lock().get(id).cloned().ok_or_else(|| ConfigError::dispatcher_unknown(id))
  }

  /// Ensures the default dispatcher entry exists.
  pub fn ensure_default(&self) {
    let mut entries = self.entries.lock();
    entries.entry(DEFAULT_DISPATCHER_ID.to_owned()).or_insert_with(DispatcherConfigGeneric::default);
  }
}

impl<TB: RuntimeToolbox + 'static> Default for DispatchersGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
