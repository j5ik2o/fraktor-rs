use alloc::{borrow::ToOwned, string::String};

use ahash::RandomState;
use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};
use hashbrown::HashMap;

use crate::core::{mailbox::MailboxRegistryError, props::MailboxConfig};

#[cfg(test)]
mod tests;

const DEFAULT_MAILBOX_ID: &str = "default";

/// Registry that manages mailbox configurations keyed by identifier.
pub struct MailboxesGeneric<TB: RuntimeToolbox + 'static> {
  entries: HashMap<String, MailboxConfig, RandomState>,
  _marker: core::marker::PhantomData<TB>,
}

/// Type alias bound to the default toolbox.
pub type Mailboxes = MailboxesGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> Clone for MailboxesGeneric<TB> {
  fn clone(&self) -> Self {
    Self { entries: self.entries.clone(), _marker: core::marker::PhantomData }
  }
}

impl<TB: RuntimeToolbox + 'static> MailboxesGeneric<TB> {
  /// Creates an empty mailbox registry.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: HashMap::with_hasher(RandomState::new()), _marker: core::marker::PhantomData }
  }

  /// Registers a mailbox configuration.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Duplicate`] when the identifier already exists.
  pub fn register(&mut self, id: impl Into<String>, config: MailboxConfig) -> Result<(), MailboxRegistryError> {
    let id = id.into();
    if self.entries.contains_key(&id) {
      return Err(MailboxRegistryError::duplicate(&id));
    }
    self.entries.insert(id, config);
    Ok(())
  }

  /// Registers or updates a mailbox configuration for the provided identifier.
  ///
  /// If the identifier already exists, the configuration is updated.
  pub fn register_or_update(&mut self, id: impl Into<String>, config: MailboxConfig) {
    self.entries.insert(id.into(), config);
  }

  /// Resolves the mailbox configuration for the provided identifier.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve(&self, id: &str) -> Result<MailboxConfig, MailboxRegistryError> {
    self.entries.get(id).copied().ok_or_else(|| MailboxRegistryError::unknown(id))
  }

  /// Ensures the default mailbox configuration is registered.
  pub fn ensure_default(&mut self) {
    self.entries.entry(DEFAULT_MAILBOX_ID.to_owned()).or_default();
  }
}

impl<TB: RuntimeToolbox + 'static> Default for MailboxesGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
