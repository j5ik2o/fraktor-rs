use alloc::{borrow::ToOwned, string::String};

use ahash::RandomState;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::sync_mutex_like::SyncMutexLike,
};
use hashbrown::HashMap;

use crate::core::{mailbox::MailboxRegistryError, props::MailboxConfig};

#[cfg(test)]
mod tests;

const DEFAULT_MAILBOX_ID: &str = "default";

/// Registry that manages mailbox configurations keyed by identifier.
pub struct MailboxesGeneric<TB: RuntimeToolbox + 'static> {
  entries: ToolboxMutex<HashMap<String, MailboxConfig, RandomState>, TB>,
}

/// Type alias bound to the default toolbox.
pub type Mailboxes = MailboxesGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> MailboxesGeneric<TB> {
  /// Creates an empty mailbox registry.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::with_hasher(RandomState::new())) }
  }

  /// Registers a mailbox configuration.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Duplicate`] when the identifier already exists.
  pub fn register(&self, id: impl Into<String>, config: MailboxConfig) -> Result<(), MailboxRegistryError> {
    let mut entries = self.entries.lock();
    let id = id.into();
    if entries.contains_key(&id) {
      return Err(MailboxRegistryError::duplicate(&id));
    }
    entries.insert(id, config);
    Ok(())
  }

  /// Resolves the mailbox configuration for the provided identifier.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve(&self, id: &str) -> Result<MailboxConfig, MailboxRegistryError> {
    self.entries.lock().get(id).copied().ok_or_else(|| MailboxRegistryError::unknown(id))
  }

  /// Ensures the default mailbox configuration is registered.
  pub fn ensure_default(&self) {
    let mut entries = self.entries.lock();
    entries.entry(DEFAULT_MAILBOX_ID.to_owned()).or_insert_with(MailboxConfig::default);
  }
}

impl<TB: RuntimeToolbox + 'static> Default for MailboxesGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
